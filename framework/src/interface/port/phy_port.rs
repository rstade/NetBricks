#![allow(dead_code)]
use super::super::{PacketRx, PacketTx};
use super::PortStats;
use crate::allocators::*;
use crate::common::errors;
use crate::common::errors::ErrorKind;
use crate::config::{DriverType, NUM_RXD, NUM_TXD, PortConfiguration};
use crate::interface::PortType::Physical;
use crate::interface::port::fdir::FlowSteeringMode;
use crate::native::zcsi::rte_ethdev_api::{RTE_ETH_FLOW_MAX, RTE_ETH_FLOW_UNKNOWN};
use crate::native::zcsi::rte_ethdev_api::{
    rte_eth_dev_info, rte_eth_dev_info_get, rte_eth_dev_rx_offload_name, rte_eth_dev_tx_offload_name,
    rte_eth_macaddr_get, rte_eth_rx_mq_mode_ETH_MQ_RX_NONE, rte_eth_rx_mq_mode_ETH_MQ_RX_RSS, rte_ether_addr, rte_flow,
};
use crate::native::zcsi::{
    MBuf, RteFdirConf, RteFlowError, add_tcp_flow, attach_device, eth_rx_burst, eth_rx_queue_count, eth_tx_burst,
    eth_tx_prepare, init_bess_eth_ring, init_ovs_eth_ring, init_pmd_port, max_rxqs, max_txqs, num_pmd_ports,
    rss_flow_name,
};
use ipnet::Ipv4Net;
use libc::if_indextoname;
use macaddr::MacAddr6 as MacAddress;
use regex::Regex;
use std::arch::x86_64::_rdtsc;
use std::cell::RefCell;
use std::cmp::min;
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::net::Ipv4Addr;
use std::process::Command;
use std::ptr;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use crate::interface::pciaddress::{PciAddress, PciError, pci_to_interface};
use crate::utils::FiveTupleV4;

#[derive(Clone, Copy, PartialEq)]
pub enum PortType {
    Physical,
    Kni,
    Virtio,
    Bess,
    Ovs,
    Null,
}

impl fmt::Display for PortType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PortType::Physical => "PHYSICAL",
            PortType::Virtio => "VIRTIO",
            PortType::Kni => "KNI",
            PortType::Bess => "BESS",
            PortType::Ovs => "OVS",
            PortType::Null => "NULL",
        };
        write!(f, "{}", s)
    }
}

#[derive(Default, Clone)]
pub struct NetSpec {
    pub mac: Option<MacAddress>,
    pub ip_net: Option<Ipv4Net>,
    pub nsname: Option<String>,
    pub port: Option<u16>,
}

pub struct PmdPort {
    name: String,
    kni_name: Option<String>,
    port_type: PortType,
    csumoffload: bool,
    port: u16,
    associated_dpdk_port_id: Option<u16>,
    linux_if: Option<String>,
    rxqs: u16,
    txqs: u16,
    pub rx_cores: Option<Vec<i32>>,
    pub tx_cores: Option<Vec<i32>>,
    n_rx_desc: u16,
    n_tx_desc: u16,
    driver: DriverType,
    stats_rx: Vec<Arc<CacheAligned<PortStats>>>,
    stats_tx: Vec<Arc<CacheAligned<PortStats>>>,
    fdir_conf: Option<RteFdirConf>,
    flow_steering_mode: Option<FlowSteeringMode>,
    net_spec: Option<NetSpec>,
}

impl fmt::Display for PmdPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}:{}, linux_if={:?})",
            self.name, self.port_type, self.port, self.linux_if
        )
    }
}

impl Default for PmdPort {
    fn default() -> PmdPort {
        PmdPort {
            name: String::new(),
            kni_name: None,
            port_type: PortType::Null,
            csumoffload: false,
            port: 0,
            associated_dpdk_port_id: None,
            linux_if: None,
            rxqs: 1,
            txqs: 1,
            rx_cores: None,
            tx_cores: None,
            n_rx_desc: 64,
            n_tx_desc: 64,
            driver: DriverType::Unknown,
            stats_rx: vec![Arc::new(PortStats::new())],
            stats_tx: vec![Arc::new(PortStats::new())],
            fdir_conf: None,
            flow_steering_mode: None,
            net_spec: None,
        }
    }
}

pub type PciQueueType = CacheAligned<PortQueueTxBuffered>;
pub type KniQueueType = CacheAligned<PortQueue>;

#[derive(Clone)]
pub struct PortQueue {
    pub port: Arc<PmdPort>,
    stats_rx: Arc<CacheAligned<PortStats>>,
    stats_tx: Arc<CacheAligned<PortStats>>,
    port_id: u16,
    txq: u16,
    rxq: u16,
}

unsafe impl Send for PortQueue {}

impl PartialEq for CacheAligned<PortQueue> {
    fn eq(&self, other: &CacheAligned<PortQueue>) -> bool {
        self.port_id == other.port_id && self.txq == other.txq && self.rxq == other.rxq
    }
}

impl Eq for CacheAligned<PortQueue> {}

impl Hash for CacheAligned<PortQueue> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.port_id.hash(state);
        self.txq.hash(state);
        self.rxq.hash(state);
    }
}

#[derive(Clone)]
pub struct PortQueueTxBuffered {
    pub port_queue: PortQueue,
    tx_queue: Rc<RefCell<TxQueue>>,
}

struct TxQueue {
    tx_buffer: VecDeque<Vec<*mut MBuf>>,
    tx_queue_len: usize,
}

impl TxQueue {
    fn with_capacity(capacity: usize) -> TxQueue {
        TxQueue {
            tx_buffer: VecDeque::with_capacity(capacity),
            tx_queue_len: 0,
        }
    }

    #[inline]
    fn push_back(&mut self, pkts: Vec<*mut MBuf>) {
        self.tx_queue_len += pkts.len();
        self.tx_buffer.push_back(pkts);
    }

    #[inline]
    fn push_front(&mut self, pkts: Vec<*mut MBuf>) {
        self.tx_queue_len += pkts.len();
        self.tx_buffer.push_front(pkts);
    }

    #[inline]
    fn pop_front(&mut self) -> Option<Vec<*mut MBuf>> {
        self.tx_buffer.pop_front().map(|pkts| {
            self.tx_queue_len -= pkts.len();
            pkts
        })
    }

    #[inline]
    fn len(&self) -> usize {
        self.tx_queue_len
    }

    #[inline]
    fn batches(&self) -> usize {
        self.tx_buffer.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.tx_buffer.is_empty()
    }
}

impl fmt::Display for PortQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl PortQueue {
    #[inline]
    fn try_send(&mut self, pkts: &mut [*mut MBuf], to_send: u32) -> u32 {
        if self.csum_offload() {
            let nb_prep = unsafe { eth_tx_prepare(self.port_id, self.txq, pkts.as_mut_ptr(), to_send as u16) };
            assert_eq!(nb_prep, to_send as u16);
        }

        let sent = unsafe { eth_tx_burst(self.port_id, self.txq, pkts.as_mut_ptr(), to_send as u16) as u32 };
        self.stats_tx.stats.fetch_add(sent as usize, Ordering::Relaxed);
        sent
    }

    #[inline]
    fn send_queue(&mut self, pkts: &mut [*mut MBuf], to_send: u32) -> errors::Result<u32> {
        Ok(self.try_send(pkts, to_send))
    }

    #[inline]
    fn recv_queue(&self, pkts: &mut [*mut MBuf], to_recv: u16) -> errors::Result<u32> {
        let start = unsafe { _rdtsc() };
        let recv = unsafe { eth_rx_burst(self.port_id, self.rxq, pkts.as_mut_ptr(), to_recv) };

        self.stats_rx.stats.fetch_add(recv as usize, Ordering::Relaxed);

        if recv > 0 {
            let elapsed = unsafe { _rdtsc() } - start;
            self.stats_rx.cycles.fetch_add(elapsed, Ordering::Relaxed);
        }

        Ok(recv)
    }

    #[inline]
    pub fn txq(&self) -> u16 {
        self.txq
    }

    #[inline]
    pub fn rxq(&self) -> u16 {
        self.rxq
    }

    #[inline]
    pub fn port_id(&self) -> u16 {
        self.port_id
    }

    #[inline]
    pub fn n_tx_desc(&self) -> u16 {
        self.port.n_tx_desc
    }

    #[inline]
    pub fn n_rx_desc(&self) -> u16 {
        self.port.n_rx_desc
    }

    #[inline]
    pub fn rx_stats(&self) -> Arc<CacheAligned<PortStats>> {
        self.stats_rx.clone()
    }

    #[inline]
    pub fn tx_stats(&self) -> Arc<CacheAligned<PortStats>> {
        self.stats_tx.clone()
    }

    #[inline]
    pub fn csum_offload(&self) -> bool {
        self.port.csumoffload
    }
}

impl PacketTx for PortQueue {
    #[inline]
    fn send(&mut self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        self.send_queue(pkts, pkts.len() as u32)
    }
}

impl PacketRx for PortQueue {
    #[inline]
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)> {
        let recv = self.recv_queue(pkts, pkts.len() as u16)?;
        Ok((recv, self.stats_rx.get_q_len() as i32))
    }

    #[inline]
    fn queued(&self) -> usize {
        let q_count = if self.port.is_physical() {
            unsafe { eth_rx_queue_count(self.port_id, self.rxq) }
        } else {
            1
        };

        if q_count < 0 {
            panic!(
                "eth_rx_queue_count failed for port_id= {} and rxq= {}",
                self.port_id, self.rxq
            );
        }

        let count = q_count as usize;
        self.stats_rx.set_q_len(count);
        count
    }
}

#[inline]
fn i32_from_bool(x: bool) -> i32 {
    i32::from(x)
}

impl PortQueueTxBuffered {
    fn queue(&mut self, pkts: &mut [*mut MBuf]) {
        let len = pkts.len();
        let pkt_vec = pkts.to_vec();
        self.tx_queue.borrow_mut().push_back(pkt_vec);
        self.port_queue.stats_tx.queued.fetch_add(len, Ordering::Relaxed);
        trace!("qlen= {}", self.tx_queue_len());
    }

    #[inline]
    fn tx_queue_len(&self) -> usize {
        self.tx_queue.borrow().len()
    }

    #[inline]
    fn tx_batches(&self) -> usize {
        self.tx_queue.borrow().batches()
    }

    #[inline]
    fn tx_queue_is_empty(&self) -> bool {
        self.tx_queue.borrow().is_empty()
    }

    #[inline]
    fn send_queue(&mut self, pkts: &mut [*mut MBuf], to_send: u32) -> errors::Result<u32> {
        let stamp = unsafe { _rdtsc() };

        if self.tx_queue_is_empty() {
            let sent = self.port_queue.try_send(pkts, to_send);
            if sent < to_send {
                self.queue(&mut pkts[sent as usize..to_send as usize]);
                trace!(
                    "txq={}, {}: sent {} of {} fresh packets, queued remaining, tx q len = {}, batches = {}",
                    self.port_queue.txq, stamp, sent, to_send, self.tx_queue_len(), self.tx_batches()
                );
            }
            return Ok(to_send);
        }

        loop {
            let mut queued_batch = self.tx_queue.borrow_mut().pop_front().unwrap();
            let len = queued_batch.len();
            let sent = self.port_queue.try_send(&mut queued_batch, len as u32) as usize;

            trace!(
                "txq={}, {}: sent {} of {} queued packets, tx q len = {}, batches= {}",
                self.port_queue.txq, stamp, sent, len, self.tx_queue_len(), self.tx_batches()
            );

            if sent < len {
                let remaining = queued_batch[sent..].to_vec();
                self.tx_queue.borrow_mut().push_front(remaining);
                self.queue(pkts);
                trace!(
                    "txq={}, {}: queuing full fresh {} packets, tx q len= {}, batches= {}",
                    self.port_queue.txq, stamp, to_send, self.tx_queue_len(), self.tx_batches()
                );
                break;
            }

            if self.tx_queue_is_empty() {
                let sent = self.port_queue.try_send(pkts, to_send);
                if sent < to_send {
                    self.queue(&mut pkts[sent as usize..to_send as usize]);
                    trace!(
                        "txq={}, {}: queuing remaining fresh {} packets, tx q len= {}, batches= {}",
                        self.port_queue.txq, stamp, to_send - sent, self.tx_queue_len(), self.tx_batches()
                    );
                }
                break;
            }
        }

        self.port_queue.stats_tx.set_q_len(self.tx_queue_len());
        Ok(to_send)
    }

    #[inline]
    pub fn rx_stats(&self) -> Arc<CacheAligned<PortStats>> {
        self.port_queue.stats_rx.clone()
    }

    #[inline]
    pub fn tx_stats(&self) -> Arc<CacheAligned<PortStats>> {
        self.port_queue.stats_tx.clone()
    }
}

impl PacketTx for PortQueueTxBuffered {
    #[inline]
    fn send(&mut self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        self.send_queue(pkts, pkts.len() as u32)
    }
}

impl PacketRx for PortQueueTxBuffered {
    #[inline]
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)> {
        self.port_queue.recv(pkts)
    }

    #[inline]
    fn queued(&self) -> usize {
        self.port_queue.queued()
    }
}

impl fmt::Display for PortQueueTxBuffered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.port_queue.fmt(f)
    }
}

fn run_command(cmd: &str, args: &[&str]) -> Result<(), String> {
    println!("  Running: {} {}", cmd, args.join(" "));

    let output = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Command failed: {}", stderr));
    }

    Ok(())
}

fn interface_exists(iface: &str) -> bool {
    Command::new("ip")
        .args(&["link", "show", iface])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn bring_interface_down(iface: &str) -> Result<(), String> {
    if !interface_exists(iface) {
        println!("  ℹ Interface {} not found; skipping 'ip link set down'", iface);
        return Ok(());
    }

    println!("📉 Bringing interface {} down...", iface);
    run_command("ip", &["link", "set", iface, "down"])?;
    println!("  ✓ Interface {} is down", iface);
    Ok(())
}

fn load_vfio_pci() -> Result<(), String> {
    println!("🔧 Loading vfio-pci module...");
    run_command("modprobe", &["vfio_pci"])?;
    println!("  ✓ vfio-pci module loaded");
    Ok(())
}

fn bind_to_vfio_pci(pci_addr: &str) -> Result<(), String> {
    println!("🔗 Binding {} to vfio-pci...", pci_addr);
    run_command("dpdk-devbind.py", &["--bind", "vfio-pci", pci_addr])?;
    println!("  ✓ Device {} bound to vfio-pci", pci_addr);
    Ok(())
}

pub fn bind_device_to_dpdk(pci_addr: &str) -> Result<(), String> {
    println!("\n🚀 DPDK Device Binding Tool");
    println!("================================\n");
    println!("PCI Address: {}", pci_addr);

    print!("🔍 Looking up interface name... ");
    match pci_to_interface(pci_addr) {
        Ok(iface) => {
            println!("found: {}", iface);
            bring_interface_down(&iface)?;
        }
        Err(PciError::NotFound) => {
            println!("not found (device may already be bound to DPDK)");
        }
        Err(e) => {
            return Err(format!("Failed to lookup interface: {}", e));
        }
    }

    load_vfio_pci()?;
    bind_to_vfio_pci(pci_addr)?;

    println!("\n✅ Successfully bound {} to DPDK (vfio-pci)\n", pci_addr);
    Ok(())
}

fn reset_pci_device(pci_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    let reset_path = format!("/sys/bus/pci/devices/{}/reset", pci_addr);
    println!("Resetting device {}...", pci_addr);
    fs::write(&reset_path, "1")?;
    sleep(Duration::from_millis(500));

    Ok(())
}

impl PmdPort {
    #[inline]
    pub fn num_pmd_ports() -> i32 {
        unsafe { num_pmd_ports() }
    }

    #[inline]
    pub fn port_id(&self) -> u16 {
        self.port
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn kni_name(&self) -> Option<&str> {
        self.kni_name.as_deref()
    }

    #[inline]
    pub fn associated_dpdk_port_id(&self) -> Option<u16> {
        self.associated_dpdk_port_id
    }

    #[inline]
    pub fn linux_if(&self) -> Option<&str> {
        self.linux_if.as_deref()
    }

    #[inline]
    pub fn port_type(&self) -> &PortType {
        &self.port_type
    }

    #[inline]
    pub fn flow_steering_mode(&self) -> &Option<FlowSteeringMode> {
        &self.flow_steering_mode
    }

    #[inline]
    pub fn net_spec(&self) -> &Option<NetSpec> {
        &self.net_spec
    }

    #[inline]
    pub fn ip_addr(&self) -> Option<Ipv4Addr> {
        self.net_spec
            .as_ref()
            .and_then(|spec| spec.ip_net)
            .map(|ip_net| ip_net.addr())
    }

    #[inline]
    pub fn rxqs(&self) -> u16 {
        self.rxqs
    }

    #[inline]
    pub fn txqs(&self) -> u16 {
        self.txqs
    }

    #[inline]
    pub fn driver(&self) -> DriverType {
        self.driver
    }

    #[inline]
    pub fn csum_offload(&self) -> bool {
        self.csumoffload
    }

    #[inline]
    pub fn get_tcp_dst_port_mask(&self) -> u16 {
        self.fdir_conf
            .map(|conf| u16::from_be(conf.mask.dst_port_mask))
            .unwrap_or(0)
    }

    #[inline]
    pub fn get_ipv4_dst_mask(&self) -> u32 {
        self.fdir_conf
            .map(|conf| u32::from_be(conf.mask.ipv4_mask.dst_ip))
            .unwrap_or(0)
    }

    #[inline]
    pub fn is_virtio(&self) -> bool {
        self.port_type == PortType::Virtio
    }

    #[inline]
    pub fn is_physical(&self) -> bool {
        self.port_type == PortType::Physical
    }

    pub fn new_queue_pair(port: &Arc<PmdPort>, rxq: u16, txq: u16) -> errors::Result<CacheAligned<PortQueue>> {
        if rxq > port.rxqs {
            return Err(ErrorKind::BadRxQueue(port.port, rxq).into());
        }
        if txq > port.txqs {
            return Err(ErrorKind::BadTxQueue(port.port, txq).into());
        }

        debug!(
            "allocating PortQueue type= {}, port_id= {}, rxq= {}, txq= {}",
            port.port_type, port.port, rxq, txq
        );

        Ok(CacheAligned::allocate(PortQueue {
            port: port.clone(),
            port_id: port.port,
            txq,
            rxq,
            stats_rx: port.stats_rx[rxq as usize].clone(),
            stats_tx: port.stats_tx[txq as usize].clone(),
        }))
    }

    pub fn new_tx_buffered_queue_pair(
        port: &Arc<PmdPort>,
        rxq: u16,
        txq: u16,
    ) -> errors::Result<CacheAligned<PortQueueTxBuffered>> {
        if rxq > port.rxqs {
            return Err(ErrorKind::BadRxQueue(port.port, rxq).into());
        }
        if txq > port.txqs {
            return Err(ErrorKind::BadTxQueue(port.port, txq).into());
        }

        debug!(
            "allocating PortQueueTxBuffered port_id= {}, rxq= {}, txq= {}",
            port.port, rxq, txq
        );

        Ok(CacheAligned::allocate(PortQueueTxBuffered {
            port_queue: PortQueue {
                port: port.clone(),
                port_id: port.port,
                txq,
                rxq,
                stats_rx: port.stats_rx[rxq as usize].clone(),
                stats_tx: port.stats_tx[txq as usize].clone(),
            },
            tx_queue: Rc::new(RefCell::new(TxQueue::with_capacity(4096))),
        }))
    }

    pub fn stats(&self, queue: u16) -> (usize, usize, usize) {
        let idx = queue as usize;
        (
            self.stats_rx[idx].stats.load(Ordering::Relaxed),
            self.stats_tx[idx].stats.load(Ordering::Relaxed),
            self.stats_rx[idx].get_max_q_len(),
        )
    }

    fn queue_stats(&self, queue: u16) -> (usize, usize, usize, usize, usize, u64) {
        let idx = queue as usize;
        (
            self.stats_rx[idx].stats.load(Ordering::Relaxed),
            self.stats_tx[idx].stats.load(Ordering::Relaxed),
            self.stats_tx[idx].queued.load(Ordering::Relaxed),
            self.stats_tx[idx].get_max_q_len(),
            self.stats_rx[idx].get_max_q_len(),
            self.stats_rx[idx].cycles(),
        )
    }

    pub fn map_rx_flow_2_queue(&self, rxq: u16, flow: FiveTupleV4, flow_mask: FiveTupleV4) -> Option<&rte_flow> {
        unsafe {
            let mut error = RteFlowError {
                err_type: 0,
                cause: ptr::null_mut(),
                message: ptr::null_mut(),
            };

            let rte_flow = add_tcp_flow(
                self.port_id(),
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

            if let Some(_) = rte_flow {
                debug!("Flow created for queue {}.", rxq);
            } else {
                let msg = error.message.as_ref()
                    .and_then(|ptr| CStr::from_ptr(ptr).to_str().ok())
                    .unwrap_or("(no stated reason)");
                error!("Flow can't be created, error type {}, message: {}\n", error.err_type, msg);
            }

            rte_flow
        }
    }

    pub fn print_soft_statistics(&self) {
        println!(
            "{0:>3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | {6: >20} |",
            "q", "rx_packets", "tx_packets", "tx_queued", "tx_q_len", "rx_q_len", "rx_cycles"
        );

        let (mut sin_p, mut sout_p, mut s_tx_queued) = (0, 0, 0);

        for q in 0..self.rxqs() {
            let (in_p, out_p, tx_queued, tx_max_q_len, rx_max_q_len, cycles) = self.queue_stats(q);
            sin_p += in_p;
            sout_p += out_p;
            s_tx_queued += tx_queued;
            println!(
                "{0:>3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | {6: >20} |",
                q, in_p, out_p, tx_queued, tx_max_q_len, rx_max_q_len, cycles
            );
        }

        println!("{0: >3} | {1: >20} | {2: >20} | {3: >20} |\n", "sum", sin_p, sout_p, s_tx_queued);
    }

    pub fn print_eth_dev_info(port: u16) {
        let mut dev_info = rte_eth_dev_info::new_null();
        unsafe {
            rte_eth_dev_info_get(port, &mut dev_info as *mut rte_eth_dev_info);
        }
        let if_index = dev_info.if_index;
        let mut buffer = Vec::<u8>::with_capacity(1024);
        let if_name = if if_index > 0 {
            unsafe {
                if_indextoname(if_index, buffer.as_mut_ptr() as *mut i8);
                CString::new(buffer).expect("if_indextoname failed")
            }
        } else {
            CString::new("-").expect("CString::new failed")
        };

        println!(
            "\nEthernet device information (port_id: {}, if_index: {}, if_name: {}, driver: {})",
            port,
            if_index,
            if_name.into_string().expect("bad if_name"),
            unsafe { CStr::from_ptr(dev_info.driver_name).to_str().expect("bad string") }
        );

        print!("   RX offload capabilities: ");
        let rx_offload_capa = dev_info.rx_offload_capa;
        for i in 0..64 {
            let offload_id = 1u64 << i;
            if offload_id & rx_offload_capa != 0 {
                let offload_capa_name = unsafe { CStr::from_ptr(rte_eth_dev_rx_offload_name(offload_id)) };
                print!("{} ", offload_capa_name.to_str().expect("bad string"));
            }
        }
        println!();

        print!("   RX per queue offload capabilities: ");
        let rx_offload_capa = dev_info.rx_queue_offload_capa;
        for i in 0..64 {
            let offload_id = 1u64 << i;
            if offload_id & rx_offload_capa != 0 {
                let offload_capa_name = unsafe { CStr::from_ptr(rte_eth_dev_rx_offload_name(offload_id)) };
                print!("{} ", offload_capa_name.to_str().expect("bad string"));
            }
        }
        println!();

        print!("   TX offload capabilities: ");
        let tx_offload_capa = dev_info.tx_offload_capa;
        for i in 0..64 {
            let offload_id = 1u64 << i;
            if offload_id & tx_offload_capa != 0 {
                let offload_capa_name = unsafe { CStr::from_ptr(rte_eth_dev_tx_offload_name(offload_id)) };
                print!("{} ", offload_capa_name.to_str().expect("bad string"));
            }
        }
        println!();

        print!("   TX per queue offload capabilities: ");
        let tx_offload_capa = dev_info.tx_queue_offload_capa;
        for i in 0..64 {
            let offload_id = 1u64 << i;
            if offload_id & tx_offload_capa != 0 {
                let offload_capa_name = unsafe { CStr::from_ptr(rte_eth_dev_tx_offload_name(offload_id)) };
                print!("{} ", offload_capa_name.to_str().expect("bad string"));
            }
        }
        println!();

        print!("   RSS offload capabilities: ");
        let rss_offload_capa = dev_info.flow_type_rss_offloads;
        for i in RTE_ETH_FLOW_UNKNOWN..RTE_ETH_FLOW_MAX {
            let offload_id = 1u64 << i;
            if offload_id & rss_offload_capa != 0 {
                let offload_capa_name = rss_flow_name(i as usize);
                print!("{} ", offload_capa_name);
            }
        }
        println!();

        let x = (dev_info.max_rx_queues, dev_info.max_tx_queues);
        println!("   Max RX/TX queues:  {} / {}", x.0, x.1);

        let x = dev_info.max_mac_addrs;
        println!("   Max MAC addresses:  {}", x);
        println!();
    }

    /// Create a PMD port with a given number of RX and TXQs.
    fn init_dpdk_port(
        name: &str,
        kni: Option<String>,
        linux_if: Option<String>,
        port: u16,
        rx_cores: &[i32],
        tx_cores: &[i32],
        nrxd: u16,
        ntxd: u16,
        loopback: bool,
        tso: bool,
        csumoffload: bool,
        driver: DriverType,
        port_type: PortType,
        rss_key: Option<&[u8]>,
        fdir_conf: Option<&RteFdirConf>,
        flow_steering_mode: Option<FlowSteeringMode>,
        net_spec: Option<NetSpec>,
        associated_dpdk_port_id: Option<u16>,
    ) -> errors::Result<Arc<PmdPort>> {
        let loopbackv = i32_from_bool(loopback);
        let tsov = i32_from_bool(tso);
        let csumoffloadv = i32_from_bool(csumoffload);
        let max_txqs = unsafe { max_txqs(port) };
        let max_rxqs = unsafe { max_rxqs(port) };
        assert!(max_rxqs >= 0);
        assert!(max_txqs >= 0);
        let rxqs = rx_cores.len() as u16;
        let txqs = tx_cores.len() as u16;
        let actual_rxqs = min(max_rxqs as u16, rxqs);
        let actual_txqs = min(max_txqs as u16, txqs);
        if actual_rxqs < rxqs || actual_txqs < txqs {
            warn!(
                "exceeding #queue limits: max_rxqs={}, max_txqs={}, using max value(s)",
                max_rxqs, max_txqs
            );
        }
        if actual_rxqs > 0 && actual_txqs > 0 {
            // DPDK no longer accepts RSS on some virtual ports like virtio
            let rx_mq_mode = if port_type == Physical {
                rte_eth_rx_mq_mode_ETH_MQ_RX_RSS
            } else {
                rte_eth_rx_mq_mode_ETH_MQ_RX_NONE
            };
            let (rss_key_ptr, rss_key_len) = rss_key.map_or((ptr::null(), 0), |key| (key.as_ptr(), key.len() as u16));
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
                    rx_mq_mode,
                    rss_key_ptr,
                    rss_key_len,
                    fdir_conf.map_or(ptr::null(), |conf| conf as *const RteFdirConf),
                )
            };
            if ret == 0 {
                Ok(Arc::new(PmdPort {
                    name: name.to_string(),
                    kni_name: kni,
                    port_type,
                    port,
                    linux_if,
                    rxqs: actual_rxqs,
                    txqs: actual_txqs,
                    rx_cores: Some(rx_cores.to_vec()),
                    tx_cores: Some(tx_cores.to_vec()),
                    n_rx_desc: nrxd,
                    n_tx_desc: ntxd,
                    csumoffload,
                    driver,
                    stats_rx: (0..actual_rxqs).map(|_| Arc::new(PortStats::new())).collect(),
                    stats_tx: (0..actual_txqs).map(|_| Arc::new(PortStats::new())).collect(),
                    fdir_conf: fdir_conf.cloned(),
                    flow_steering_mode,
                    net_spec,
                    associated_dpdk_port_id,
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
                name: name.to_string(),
                kni_name: None,
                port_type: PortType::Bess,
                port: port as u16,
                ..Default::default()
            }))
        } else {
            Err(ErrorKind::FailedToInitializeBessPort(port).into())
        }
    }

    fn new_ovs_port(name: &str, core: i32) -> errors::Result<Arc<PmdPort>> {
        match name.parse() {
            Ok(iface) => {
                // This call returns the port number
                let port = unsafe { init_ovs_eth_ring(iface, core) };
                if port >= 0 {
                    Ok(Arc::new(PmdPort {
                        name: name.to_string(),
                        kni_name: None,
                        port_type: PortType::Ovs,
                        port: port as u16,
                        ..Default::default()
                    }))
                } else {
                    Err(ErrorKind::FailedToInitializeOvsPort(port).into())
                }
            }
            _ => Err(ErrorKind::BadVdev(String::from(name)).into()),
        }
    }

    fn new_dpdk_port(
        name: &str,
        kni: Option<String>,
        linux_if: Option<String>,
        spec: &str,
        rx_cores: &[i32],
        tx_cores: &[i32],
        nrxd: u16,
        ntxd: u16,
        loopback: bool,
        tso: bool,
        csumoffload: bool,
        driver: DriverType,
        port_type: PortType,
        rss_key: Option<&[u8]>,
        fdir_conf: Option<&RteFdirConf>,
        flow_steering_mode: Option<FlowSteeringMode>,
        net_spec: Option<NetSpec>,
        associated_dpdk_port_id: Option<u16>,
    ) -> errors::Result<Arc<PmdPort>> {
        let canonical_spec = PmdPort::canonicalize_pci(spec);

        // Bind and reset physical PCI device if applicable
        if PciAddress::parse(spec).is_ok() {
            bind_device_to_dpdk(spec)
                .map_err(|e| ErrorKind::BadDev(format!("Failed to bind PCI device to dpdk: {}", e)))?;

            reset_pci_device(canonical_spec.to_str().unwrap())
                .map_err(|e| ErrorKind::BadDev(format!("Failed to reset PCI device: {}", e)))?;
        }

        // Attach device and get port ID
        debug!("attach_pmd_device, port = {:?}", canonical_spec);
        let mut ports: Vec<u16> = Vec::with_capacity(16);
        let rc = unsafe { attach_device(canonical_spec.as_ptr(), ports.as_mut_ptr(), 16) };

        if rc < 0 {
            return Err(ErrorKind::BadDev(String::from(spec)).into());
        }

        unsafe { ports.set_len(rc as usize); }

        if rc > 1 {
            warn!(
            "dpdk detected {} ports for spec {}, using first port with id {}",
            rc, spec, ports[0]
        );
        }

        let port = ports[0];
        debug!("Going to initialize dpdk port {} ({})", port, spec);

        PmdPort::init_dpdk_port(
            name,
            kni,
            linux_if,
            port,
            rx_cores,
            tx_cores,
            nrxd,
            ntxd,
            loopback,
            tso,
            csumoffload,
            driver,
            port_type,
            rss_key,
            fdir_conf,
            flow_steering_mode,
            net_spec,
            associated_dpdk_port_id,
        )
    }

    fn null_port() -> errors::Result<Arc<PmdPort>> {
        Ok(Arc::new(PmdPort {
            name: String::from("NullPort"),
            kni_name: None,
            port_type: PortType::Null,
            port: 0,
            ..Default::default()
        }))
    }

    /// Create a new port from a `PortConfiguration`.
    pub fn new_port_from_configuration(
        port_config: &PortConfiguration,
        associated_port: Option<&Arc<PmdPort>>,
        rss_keys: &Option<Vec<Vec<u8>>>,
    ) -> errors::Result<Arc<PmdPort>> {
        /// Create a new port.
        ///
        /// Description
        /// -   `name`: The name for a port. NetBricks currently supports Bess native vports, OVS shared memory ports and
        ///     `dpdk` PMDs. DPDK PMDs can be used to input pcap (e.g., `dpdk:eth_pcap0,rx_pcap=<pcap_name>`), etc.
        /// -   `rxqs`, `txqs`: Number of RX and TX queues.
        /// -   `tx_cores`, `rx_cores`: Core affinity of where the queues will be used.
        /// -   `nrxd`, `ntxd`: RX and TX descriptors.
        let name = &port_config.name[..];
        let rx_cores = &port_config.rx_queues[..];
        let tx_cores = &port_config.tx_queues[..];
        let nrxd = port_config.rxd;
        let ntxd = port_config.txd;
        let loopback = port_config.loopback;
        let tso = port_config.tso;
        let csumoffload = port_config.csum;
        let driver = port_config.driver;
        let rss_key_idx = port_config.rss_key;  // index to rss_keys
        let fdir_conf = port_config.fdir_conf.as_ref();
        let kni = port_config.kni.clone();
        let parts: Vec<_> = name.splitn(2, ':').collect();
        let queues = associated_port.map_or(Some(rx_cores.len()), |p| Some(p.rx_cores.as_ref().unwrap().len()));
        
        #[derive(Debug)]
        struct DevSpec {
            name: String,
            iface: Option<String>,
            path: Option<String>,
            queue_size: Option<u32>,
            queues: Option<u32>,
            rx_pcap: Option<String>,
            tx_pcap: Option<String>,
        }

        fn parse_spec(spec: &str) -> DevSpec {
            let mut iface = None;
            let mut path = None;
            let mut queue_size = None;
            let mut queues = None;
            let mut rx_pcap = None;
            let mut tx_pcap = None;
            let mut name = String::new();

            for (i, s) in spec.split_terminator(',').enumerate() {
                if i == 0 {
                    // we take as name key everything before the first ','
                    name = s.to_string();
                } else {
                    let key_val: Vec<_> = s.split_terminator('=').collect();
                    if key_val.len() == 2 {
                        match key_val[0] {
                            "iface" => iface = Some(key_val[1].to_string()),
                            "path" => path = Some(key_val[1].to_string()),
                            "rx_pcap" => rx_pcap = Some(key_val[1].to_string()),
                            "tx_pcap" => tx_pcap = Some(key_val[1].to_string()),
                            "queue_size" => queue_size = key_val[1].parse::<u32>().ok(),
                            "queues" => queues = key_val[1].parse::<u32>().ok(),
                            _ => (),
                        }
                    } else {
                        debug!("ignoring attribute {} found in {}", s, spec);
                    }
                }
            }

            DevSpec {
                name,
                iface,
                path,
                queue_size,
                queues,
                rx_pcap,
                tx_pcap,
            }
        }

        let selected_rss_key: Option<&[u8]> = match (rss_keys, rss_key_idx) {
            (Some(keys), Some(idx)) => {
                if idx < keys.len() {
                    debug!("using rss key {} from rss_keys", idx);
                    Some(&keys[idx][..])
                } else {
                    warn!("RSS key index {} out of bounds (max {}), using None", idx, keys.len() - 1);
                    None
                }
            },
            _ => None,
        };

        match parts[0] {
            "bess" => PmdPort::new_bess_port(parts[1], rx_cores[0]),
            "ovs" => PmdPort::new_ovs_port(parts[1], rx_cores[0]),
            "virtio" | "dpdk" => {
                let port_type = match parts[0] {
                    "dpdk" => PortType::Physical,
                    "virtio" => PortType::Virtio,
                    _ => PortType::Null,
                };
                let dev_spec = parse_spec(name);
                debug!("spec {} parsed as {:?}", parts[1], dev_spec);
                // we must have for each core of the associated port a queue on the virtio device
                let modified_spec = if queues.is_some() {
                    parts[1].replace("queues={}", &("queues=".to_owned() + &format!("{}", queues.unwrap())))
                } else {
                    parts[1].to_string()
                };
                debug!("modified spec= {}", modified_spec);
                 PmdPort::new_dpdk_port(
                    &dev_spec.name,
                    kni,
                    dev_spec.iface,
                    &modified_spec,
                    associated_port.map_or(rx_cores, |p| &p.rx_cores.as_ref().unwrap()[..]),
                    associated_port.map_or(tx_cores, |p| &p.tx_cores.as_ref().unwrap()[..]),
                    nrxd,
                    ntxd,
                    loopback,
                    tso,
                    csumoffload,
                    driver,
                    port_type,
                    selected_rss_key,
                    fdir_conf,
                    port_config.flow_steering,
                    port_config.net_spec.clone(),
                    associated_port.map_or(None, |p| Some(p.port_id())),
                )
            }
            "null" => PmdPort::null_port(),
            _ => PmdPort::new_dpdk_port(
                name,
                kni,
                None,
                name,
                rx_cores,
                tx_cores,
                nrxd,
                ntxd,
                loopback,
                tso,
                csumoffload,
                driver,
                PortType::Physical,
                selected_rss_key,
                fdir_conf,
                port_config.flow_steering,
                None,
                associated_port.map_or(None, |p| Some(p.port_id())),
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
            rss_key: None,
            fdir_conf: None,
            flow_steering: None,
            kni: None,
            driver: DriverType::Unknown,
            net_spec: None,
        };
        PmdPort::new_port_from_configuration(&config, None, &None)
    }
/*  do we need this?
    pub fn new_with_cores(name: &str, rx_core: i32, tx_core: i32) -> errors::Result<Arc<PmdPort>> {
        let rx_vec = vec![rx_core];
        let tx_vec = vec![tx_core];
        PmdPort::new_with_queues(name, 1, 1, &rx_vec[..], &tx_vec[..])
    }

    pub fn new(name: &str, core: i32) -> errors::Result<Arc<PmdPort>> {
        PmdPort::new_with_cores(name, core, core)
    }
*/
    fn canonicalize_pci(pci: &str) -> CString {
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
        let mut address: rte_ether_addr = rte_ether_addr { addr_bytes: [0u8; 6] };
        unsafe {
            rte_eth_macaddr_get(self.port, &mut address);
        }
        MacAddress::from(address.addr_bytes)
    }
}
