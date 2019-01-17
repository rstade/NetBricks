use super::super::{PacketRx, PacketTx};
use super::PortStats;
use allocators::*;
use common::errors;
use common::errors::{ErrorKind, ResultExt};
use config::{DriverType, PortConfiguration, NUM_RXD, NUM_TXD};
use eui48::MacAddress;
use native::zcsi::*;
use regex::Regex;
use std::cmp::min;
use std::ffi::{CStr, CString};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ptr;
use std::ptr::Unique;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use utils::{FiveTupleV4, rdtsc_unsafe};

/// A DPDK based PMD port. Send and receive should not be called directly on this structure but on the port queue
/// structure instead.
#[derive(Clone, Copy, PartialEq)]
pub enum PortType {
    Dpdk,
    Kni,
    Bess,
    Ovs,
    Null,
}

impl fmt::Display for PortType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::PortType::*;
        let printable = match *self {
            Dpdk => "DPDK",
            Kni => "KNI",
            Bess => "BESS",
            Ovs => "OVS",
            Null => "NULL",
        };
        write!(f, "{}", printable)
    }
}

pub struct PmdPort {
    port_type: PortType,
    connected: bool,
    should_close: bool,
    csumoffload: bool,
    port: i32,
    kni: Option<Unique<RteKni>>,
    //must use Unique because raw ptr does not implement Send
    linux_if: Option<String>,
    // used for kni interfaces
    rxqs: i32,
    txqs: i32,
    driver: DriverType,
    stats_rx: Vec<Arc<CacheAligned<PortStats>>>,
    stats_tx: Vec<Arc<CacheAligned<PortStats>>>,
    fdir_conf: Option<RteFdirConf>,
}

impl fmt::Display for PmdPort {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.port_type, self.port)
    }
}

/// A port queue represents a single queue for a physical port, and should be used to send and receive data.
#[derive(Clone)]
pub struct PortQueue {
    // The Arc cost here should not affect anything, since we are really not doing anything to make it go in and out of
    // scope.
    pub port: Arc<PmdPort>,
    stats_rx: Arc<CacheAligned<PortStats>>,
    stats_tx: Arc<CacheAligned<PortStats>>,
    port_id: i32,
    txq: i32,
    rxq: i32,
}

impl PartialEq for CacheAligned<PortQueue> {
    fn eq(&self, other: &CacheAligned<PortQueue>) -> bool {
        self.port_id == other.port_id
            && self.txq == other.txq
            && self.rxq == other.rxq
            && self.port.is_kni() == other.port.is_kni()
    }
}

impl Eq for CacheAligned<PortQueue> {}

impl Hash for CacheAligned<PortQueue> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.port_id.hash(state);
        self.txq.hash(state);
        self.rxq.hash(state);
        self.port.is_kni().hash(state);
    }
}

impl Drop for PmdPort {
    fn drop(&mut self) {
        if self.connected && self.should_close {
            unsafe {
                free_pmd_port(self.port);
            }
        }
    }
}

/// Print information about PortQueue
impl fmt::Display for PortQueue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "port: {} ({}) rxq: {} txq: {}, max_rxq_len: {}, recv_cycles: {}",
            self.port.mac_address(),
            self.port_id,
            self.rxq,
            self.txq,
            self.stats_rx.get_max_q_len(),
            self.stats_rx.cycles(),
        )
    }
}

/// Represents a single RX/TX queue pair for a port. This is what is needed to send or receive traffic.
impl PortQueue {
    #[inline]
    fn send_queue(&self, queue: i32, pkts: *mut *mut MBuf, to_send: u16) -> errors::Result<u32> {
        unsafe {
            let sent = if self.port.is_kni() {
                rte_kni_tx_burst(self.port.kni.unwrap().as_ptr(), pkts, to_send as u32)
            } else {
                if self.csum_offload() {
                    let nb_prep= eth_tx_prepare(self.port_id as u16, queue as u16, pkts, to_send);
                    assert_eq!(nb_prep, to_send);
                }
                eth_tx_burst(self.port_id as u16, queue as u16, pkts, to_send) as u32
            };
            let update = self.stats_tx.stats.load(Ordering::Relaxed) + sent as usize;
            self.stats_tx.stats.store(update, Ordering::Relaxed);
            Ok(sent as u32)
        }
    }

    #[inline]
    fn recv_queue(&self, pkts: &mut [*mut MBuf], to_recv: u16) -> errors::Result<(u32, i32)> {
        let start= rdtsc_unsafe();
        unsafe {
            let (recv, q_count) = if self.port.is_kni() {
                //debug!("calling rte_kni_rx_burst for {}.{}", self.port, self.rxq);
                (rte_kni_rx_burst(self.port.kni.unwrap().as_ptr(), pkts.as_mut_ptr(), to_recv as u32), 0)
            } else {
                //debug!("calling eth_rx_burst for {}.{}", self.port, self.rxq);
                (eth_rx_burst(self.port_id, self.rxq, pkts.as_mut_ptr(), to_recv),
                eth_rx_queue_count(self.port_id as u16, self.rxq as u16))
            };
            //debug!("received { } packets", recv);
            let update = self.stats_rx.stats.load(Ordering::Relaxed) + recv as usize;
            self.stats_rx.stats.store(update, Ordering::Relaxed);
            self.stats_rx.set_q_len(q_count as usize);
            if recv > 0 ||q_count > 0 {
                let update = self.stats_rx.cycles.load(Ordering::Relaxed) + (rdtsc_unsafe() - start);
                self.stats_rx.cycles.store(update, Ordering::Relaxed);
            }
            Ok((recv, q_count))
        }
    }

    #[inline]
    pub fn txq(&self) -> u16 {
        self.txq as u16
    }

    #[inline]
    pub fn rxq(&self) -> u16 {
        self.rxq as u16
    }

    #[inline]
    pub fn rx_stats(&self) -> Arc<CacheAligned<PortStats>> { self.stats_rx.clone() }

    #[inline]
    pub fn tx_stats(&self) -> Arc<CacheAligned<PortStats>> { self.stats_tx.clone() }

    #[inline]
    pub fn csum_offload(&self) -> bool { self.port.csumoffload }
}

impl PacketTx for PortQueue {
    /// Send a batch of packets out this PortQueue. Note this method is internal to NetBricks (should not be directly
    /// called).
    #[inline]
    fn send(&self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        let txq = self.txq;
        let len = pkts.len() as u16;
        self.send_queue(txq, pkts.as_mut_ptr(), len)
    }

    fn port_id(&self) -> i32 {
        self.port.port_id()
    }
}

impl PacketRx for PortQueue {
    /// Receive a batch of packets out this PortQueue. Note this method is internal to NetBricks (should not be directly
    /// called).
    #[inline]
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)> {
        let len = pkts.len() as u16;
        self.recv_queue(pkts, len)
    }

    fn port_id(&self) -> i32 {
        self.port.port_id()
    }

    fn queued(&self) -> usize { self.stats_rx.get_q_len() }
}

// Utility function to go from Rust bools to C ints. Allowing match bools since this looks nicer to me.
#[cfg_attr(feature = "dev", allow(match_bool))]
#[inline]
fn i32_from_bool(x: bool) -> i32 {
    match x {
        true => 1,
        false => 0,
    }
}

impl PmdPort {
    /// Determine the number of ports in a system.
    pub fn num_pmd_ports() -> i32 {
        unsafe { num_pmd_ports() }
    }

    /// Find a port ID given a PCI-E string.
    pub fn find_port_id(pcie: &str) -> i32 {
        let pcie_cstr = CString::new(pcie).unwrap();
        unsafe { find_port_with_pci_address(pcie_cstr.as_ptr()) }
    }

    pub fn port_id(&self) -> i32 {
        self.port
    }

    pub fn linux_if(&self) -> Option<&String> {
        self.linux_if.as_ref()
    }

    pub fn port_type(&self) -> &PortType {
        &self.port_type
    }

    /// Number of configured RXQs.
    pub fn rxqs(&self) -> i32 {
        self.rxqs
    }

    /// Number of configured TXQs.
    pub fn txqs(&self) -> i32 {
        self.txqs
    }

    pub fn driver(&self) -> DriverType {
       self.driver
    }

    pub fn csum_offload(&self) -> bool { self.csumoffload }

    pub fn get_tcp_dst_port_mask(&self) -> u16 {
        if self.fdir_conf.is_some() {
            u16::from_be(self.fdir_conf.unwrap().mask.dst_port_mask)
        } else {
            0x0000
        }
    }

    pub fn is_kni(&self) -> bool {
        self.kni.is_some()
    }

    pub fn get_kni(&self) -> *mut RteKni {
        self.kni.unwrap().as_ptr()
    }

    pub fn new_queue_pair(port: &Arc<PmdPort>, rxq: i32, txq: i32) -> errors::Result<CacheAligned<PortQueue>> {
        if rxq > port.rxqs || rxq < 0 {
            Err(ErrorKind::BadRxQueue(port.port, rxq).into())
        } else if txq > port.txqs || txq < 0 {
            Err(ErrorKind::BadTxQueue(port.port, txq).into())
        } else {
            Ok(CacheAligned::allocate(PortQueue {
                port: port.clone(),
                port_id: port.port,
                txq,
                rxq,
                stats_rx: port.stats_rx[rxq as usize].clone(),
                stats_tx: port.stats_tx[txq as usize].clone(),
            }))
        }
    }

    /// Get stats for an RX/TX queue pair.
    pub fn stats(&self, queue: i32) -> (usize, usize, usize) {
        let idx = queue as usize;
        (
            self.stats_rx[idx].stats.load(Ordering::Relaxed),
            self.stats_tx[idx].stats.load(Ordering::Relaxed),
            self.stats_rx[idx].get_max_q_len(),
        )
    }

    /// Get stats for an RX/TX queue pair.
    fn stats_4(&self, queue: i32) -> (usize, usize, usize, u64) {
        let idx = queue as usize;
        (
            self.stats_rx[idx].stats.load(Ordering::Relaxed),
            self.stats_tx[idx].stats.load(Ordering::Relaxed),
            self.stats_rx[idx].get_max_q_len(),
            self.stats_rx[idx].cycles(),
        )
    }


    pub fn map_rx_flow_2_queue(&self, rxq: u16, flow: FiveTupleV4, flow_mask: FiveTupleV4) -> Option<&RteFlow> {
        unsafe {
            let mut error = RteFlowError {
                err_type: 0,
                cause: ptr::null_mut(),
                message: ptr::null_mut(),
            };

            let rte_flow = add_tcp_flow(
                self.port_id() as u16,
                rxq,
                flow.src_ip,
                flow_mask.src_ip,
                flow.dst_ip,
                flow_mask.dst_ip,
                flow.src_port,
                flow_mask.src_port,
                flow.dst_port,
                flow_mask.dst_port,
                &mut error,
            ).as_ref();

            if rte_flow.is_none() {
                error!(
                    "Flow can't be created, error type {}, message: {}\n",
                    error.err_type,
                    match error.message.as_ref() {
                        None => "(no stated reason)",
                        Some(char_ptr) => CStr::from_ptr(char_ptr).to_str().unwrap(),
                    }
                );
            } else {
                debug!("Flow created for queue {}.", rxq);
            };
            rte_flow
        }
    }

     pub fn print_soft_statistics(&self) {
        println!(
            "{0:>3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | {6: >20} | {7: >20} | {8: >20}",
            "q", "ipackets", "opackets", "ibytes", "obytes", "ierrors", "oerrors", "queue_len", "rx_cycles"
        );
        let (mut sin_p, mut sout_p) = (0usize, 0usize);
        for q in 0..self.rxqs() {
            let (in_p, out_p, rx_max_q_len, cycles) = self.stats_4(q);
            sin_p += in_p;
            sout_p += out_p;
            println!(
                "{0:>3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | {6: >20} | {7: >20} | {8: >20}",
                q, in_p, out_p, 0, 0, 0, 0, rx_max_q_len, cycles
            );
        }
        println!(
            "{0: >3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | {6: >20}\n",
            "sum", sin_p, sout_p, 0, 0, 0, 0,
        );
    }

    /// Create a PMD port with a given number of RX and TXQs.
    fn init_dpdk_port(
        port: i32,
        rxqs: i32,
        txqs: i32,
        rx_cores: &[i32],
        tx_cores: &[i32],
        nrxd: i32,
        ntxd: i32,
        loopback: bool,
        tso: bool,
        csumoffload: bool,
        driver: DriverType,
        fdir_conf: Option<&RteFdirConf>,
    ) -> errors::Result<Arc<PmdPort>> {
        let loopbackv = i32_from_bool(loopback);
        let tsov = i32_from_bool(tso);
        let csumoffloadv = i32_from_bool(csumoffload);
        let max_txqs = unsafe { max_txqs(port) };
        let max_rxqs = unsafe { max_rxqs(port) };
        let actual_rxqs = min(max_rxqs, rxqs);
        let actual_txqs = min(max_txqs, txqs);
        debug!("max_rxqs={}, max_txqs={}", max_rxqs, max_txqs);
        if ((actual_txqs as usize) <= tx_cores.len()) && ((actual_rxqs as usize) <= rx_cores.len()) {
            debug!("calling init_pmd_port with fdir_conf {}", fdir_conf.unwrap());
            let ret = unsafe {
                init_pmd_port(
                    port,
                    actual_rxqs,
                    actual_txqs,
                    rx_cores.as_ptr(),
                    tx_cores.as_ptr(),
                    nrxd,
                    ntxd,
                    loopbackv,
                    tsov,
                    csumoffloadv,
                    if fdir_conf.is_some() {
                        fdir_conf.unwrap() as *const RteFdirConf
                    } else {
                        ptr::null()
                    },
                )
            };
            if ret == 0 {
                Ok(Arc::new(PmdPort {
                    port_type: PortType::Dpdk,
                    connected: true,
                    port,
                    kni: None,
                    linux_if: None,
                    rxqs: actual_rxqs,
                    txqs: actual_txqs,
                    should_close: true,
                    csumoffload,
                    driver,
                    stats_rx: (0..rxqs).map(|_| Arc::new(PortStats::new())).collect(),
                    stats_tx: (0..txqs).map(|_| Arc::new(PortStats::new())).collect(),
                    fdir_conf: if fdir_conf.is_some() {
                        Some(fdir_conf.unwrap().clone())
                    } else {
                        None
                    },
                }))
            } else {
                Err(ErrorKind::FailedToInitializePort(port).into())
            }
        } else {
            Err(ErrorKind::FailedToInitializePort(port).into())
        }
    }

    /// Create a new port that can talk to BESS.
    fn new_bess_port(name: &str, core: i32) -> errors::Result<Arc<PmdPort>> {
        let ifname = CString::new(name).unwrap();
        // This call returns the port number
        let port = unsafe {
            // This bit should not be required, but is an unfortunate problem with DPDK today.
            init_bess_eth_ring(ifname.as_ptr(), core)
        };
        // TODO: Can we really not close?
        if port >= 0 {
            Ok(Arc::new(PmdPort {
                port_type: PortType::Bess,
                connected: true,
                port,
                kni: None,
                linux_if: None,
                rxqs: 1,
                txqs: 1,
                should_close: false,
                csumoffload: false,
                driver: DriverType::Unknown,
                stats_rx: vec![Arc::new(PortStats::new())],
                stats_tx: vec![Arc::new(PortStats::new())],
                fdir_conf: None,
            }))
        } else {
            Err(ErrorKind::FailedToInitializePort(port).into())
        }
    }

    fn new_ovs_port(name: &str, core: i32) -> errors::Result<Arc<PmdPort>> {
        match name.parse() {
            Ok(iface) => {
                // This call returns the port number
                let port = unsafe { init_ovs_eth_ring(iface, core) };
                if port >= 0 {
                    Ok(Arc::new(PmdPort {
                        port_type: PortType::Ovs,
                        connected: true,
                        port,
                        kni: None,
                        linux_if: None,
                        rxqs: 1,
                        txqs: 1,
                        should_close: false,
                        csumoffload: false,
                        driver: DriverType::Unknown,
                        stats_rx: vec![Arc::new(PortStats::new())],
                        stats_tx: vec![Arc::new(PortStats::new())],
                        fdir_conf: None,
                    }))
                } else {
                    Err(ErrorKind::FailedToInitializePort(port).into())
                }
            }
            _ => Err(ErrorKind::BadVdev(String::from(name)).into()),
        }
    }

    fn new_kni_port(kni_port_params: Box<KniPortParams>) -> errors::Result<Arc<PmdPort>> {
        // This call returns a pointer to an opaque C struct
        let port_id = kni_port_params.port_id;
        let p_kni_port_params: *mut KniPortParams = Box::into_raw(kni_port_params);
        unsafe {
            let p_kni = kni_alloc(port_id, p_kni_port_params);
            if !p_kni.is_null() {
                Ok(Arc::new(PmdPort {
                    port_type: PortType::Kni,
                    connected: true,
                    port: port_id as i32,
                    kni: Some(Unique::new(p_kni).unwrap()),
                    linux_if: kni_get_name(p_kni),
                    rxqs: 1,
                    txqs: 1,
                    should_close: true, // sta, not clear what this is used for, and if to set true or false
                    csumoffload: false,
                    driver: DriverType::Unknown,
                    stats_rx: (0..1).map(|_| Arc::new(PortStats::new())).collect(),
                    stats_tx: (0..1).map(|_| Arc::new(PortStats::new())).collect(),
                    fdir_conf: None,
                }))
            } else {
                Err(ErrorKind::FailedToInitializeKni(port_id).into())
            }
        }
    }

    fn new_dpdk_port(
        spec: &str,
        rxqs: i32,
        txqs: i32,
        rx_cores: &[i32],
        tx_cores: &[i32],
        nrxd: i32,
        ntxd: i32,
        loopback: bool,
        tso: bool,
        csumoffload: bool,
        driver: DriverType,
        fdir_conf: Option<&RteFdirConf>,
    ) -> errors::Result<Arc<PmdPort>> {
        let cannonical_spec = PmdPort::cannonicalize_pci(spec);
        debug!("attach_pmd_device, port = {:?}", cannonical_spec);
        let port = unsafe { attach_pmd_device((cannonical_spec[..]).as_ptr()) };
        if port >= 0 {
            debug!("Going to initialize dpdk port {} ({})", port, spec);
            PmdPort::init_dpdk_port(
                port,
                rxqs,
                txqs,
                rx_cores,
                tx_cores,
                nrxd,
                ntxd,
                loopback,
                tso,
                csumoffload,
                driver,
                fdir_conf,
            ).chain_err(|| ErrorKind::BadDev(String::from(spec)))
        } else {
            Err(ErrorKind::BadDev(String::from(spec)).into())
        }
    }

    fn null_port() -> errors::Result<Arc<PmdPort>> {
        Ok(Arc::new(PmdPort {
            port_type: PortType::Null,
            connected: false,
            port: 0,
            kni: None,
            linux_if: None,
            rxqs: 0,
            txqs: 0,
            should_close: false,
            csumoffload: false,
            driver: DriverType::Unknown,
            stats_rx: vec![Arc::new(PortStats::new())],
            stats_tx: vec![Arc::new(PortStats::new())],
            fdir_conf: None,
        }))
    }

    /// Create a new port from a `PortConfiguration`.
    pub fn new_port_from_configuration(port_config: &PortConfiguration) -> errors::Result<Arc<PmdPort>> {
        /// Create a new port.
        ///
        /// Description
        /// -   `name`: The name for a port. NetBricks currently supports Bess native vports, OVS shared memory ports and
        ///     `dpdk` PMDs. DPDK PMDs can be used to input pcap (e.g., `dpdk:eth_pcap0,rx_pcap=<pcap_name>`), etc.
        /// -   `rxqs`, `txqs`: Number of RX and TX queues.
        /// -   `tx_cores`, `rx_cores`: Core affinity of where the queues will be used.
        /// -   `nrxd`, `ntxd`: RX and TX descriptors.
        let name = &port_config.name[..];
        let rxqs = port_config.rx_queues.len() as i32;
        let txqs = port_config.tx_queues.len() as i32;
        let rx_cores = &port_config.rx_queues[..];
        let tx_cores = &port_config.tx_queues[..];
        let nrxd = port_config.rxd;
        let ntxd = port_config.txd;
        let loopback = port_config.loopback;
        let tso = port_config.tso;
        let csumoffload = port_config.csum;
        let driver = port_config.driver;
        let fdir_conf = port_config.fdir_conf.as_ref();

        let parts: Vec<_> = name.splitn(2, ':').collect();
        match parts[0] {
            "bess" => PmdPort::new_bess_port(parts[1], rx_cores[0]),
            "ovs" => PmdPort::new_ovs_port(parts[1], rx_cores[0]),
            "dpdk" => PmdPort::new_dpdk_port(
                parts[1],
                rxqs,
                txqs,
                rx_cores,
                tx_cores,
                nrxd,
                ntxd,
                loopback,
                tso,
                csumoffload,
                driver,
                fdir_conf,
            ),
            "kni" => {
                let port_id: u16 = parts[1]
                    .parse::<u16>()
                    .expect(&format!("cannot parse port_id from {} as an u16", name));

                PmdPort::new_kni_port(Box::new(KniPortParams::new(
                    port_id,
                    rx_cores[0] as u32,
                    tx_cores[0] as u32,
                    &port_config.k_cores,
                )))
            }
            "null" => PmdPort::null_port(),
            _ => PmdPort::new_dpdk_port(
                name,
                rxqs,
                txqs,
                rx_cores,
                tx_cores,
                nrxd,
                ntxd,
                loopback,
                tso,
                csumoffload,
                driver,
                fdir_conf,
            ),
        }
    }

    pub fn new_with_queues(
        name: &str,
        rxqs: i32,
        txqs: i32,
        rx_cores: &[i32],
        tx_cores: &[i32],
    ) -> errors::Result<Arc<PmdPort>> {
        let config = PortConfiguration {
            name: name.to_string(),
            rx_queues: rx_cores[0..rxqs as usize].to_vec(),
            tx_queues: tx_cores[0..txqs as usize].to_vec(),
            rxd: NUM_RXD,
            txd: NUM_TXD,
            loopback: false,
            tso: false,
            csum: false,
            k_cores: vec![],
            fdir_conf: None,
            driver: DriverType::Unknown,
        };
        PmdPort::new_port_from_configuration(&config)
    }

    pub fn new_with_cores(name: &str, rx_core: i32, tx_core: i32) -> errors::Result<Arc<PmdPort>> {
        let rx_vec = vec![rx_core];
        let tx_vec = vec![tx_core];
        PmdPort::new_with_queues(name, 1, 1, &rx_vec[..], &tx_vec[..])
    }

    pub fn new(name: &str, core: i32) -> errors::Result<Arc<PmdPort>> {
        PmdPort::new_with_cores(name, core, core)
    }

    fn cannonicalize_pci(pci: &str) -> CString {
        lazy_static! {
            static ref PCI_RE: Regex = Regex::new(r"^\d{2}:\d{2}\.\d$").unwrap();
        }
        if PCI_RE.is_match(pci) {
            CString::new(format!("0000:{}", pci)).unwrap()
        } else {
            CString::new(String::from(pci)).unwrap()
        }
    }

    #[inline]
    pub fn mac_address(&self) -> MacAddress {
        let mut address = MacAddress::nil();
        unsafe {
            rte_eth_macaddr_get(self.port, &mut address as *mut MacAddress);
            address
        }
    }
}
