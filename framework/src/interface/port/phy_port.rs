#![allow(dead_code)]
use super::super::{PacketRx, PacketTx};
use super::PortStats;
use allocators::*;
use common::errors;
use common::errors::ErrorKind;
use config::{DriverType, PortConfiguration, NUM_RXD, NUM_TXD};
use eui48::MacAddress;
use interface::port::fdir::FlowSteeringMode;
use interface::PortType::Physical;
use ipnet::Ipv4Net;
use libc::if_indextoname;
use native::zcsi::rte_ethdev_api::{
    rte_eth_dev_info, rte_eth_dev_info_get, rte_eth_dev_rx_offload_name, rte_eth_dev_tx_offload_name,
    rte_eth_macaddr_get, rte_eth_rx_mq_mode_ETH_MQ_RX_NONE, rte_eth_rx_mq_mode_ETH_MQ_RX_RSS, rte_ether_addr, rte_flow,
};
use native::zcsi::rte_ethdev_api::{RTE_ETH_FLOW_MAX, RTE_ETH_FLOW_UNKNOWN};
use native::zcsi::{
    add_tcp_flow, attach_device, eth_rx_burst, eth_rx_queue_count, eth_tx_burst, eth_tx_prepare, init_bess_eth_ring,
    init_ovs_eth_ring, init_pmd_port, kni_alloc, kni_get_name, max_rxqs, max_txqs, num_pmd_ports, rss_flow_name,
    rte_kni_rx_burst, rte_kni_tx_burst, KniPortParams, MBuf, RteFdirConf, RteFlowError, RteKni,
};
use regex::Regex;
use std::arch::x86_64::_rdtsc;
use std::cell::RefCell;
use std::cmp::min;
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::net::Ipv4Addr;
use std::ptr;
use std::ptr::Unique;
use std::rc::Rc;
use std::string::ToString;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use utils::FiveTupleV4;

/// A DPDK based PMD port. Send and receive should not be called directly on this structure but on the port queue
/// structure instead.
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::PortType::*;
        let printable = match *self {
            Physical => "PHYSICAL",
            Virtio => "VIRTIO",
            Kni => "KNI",
            Bess => "BESS",
            Ovs => "OVS",
            Null => "NULL",
        };
        write!(f, "{}", printable)
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
    // id of an associated port, if any
    associated_dpdk_port_id: Option<u16>,
    //must use Unique because raw ptr does not implement Send
    kni: Option<Unique<RteKni>>,
    // used for kni interfaces
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
            //must use Unique because raw ptr does not implement Send
            kni: None,
            // used for kni interfaces
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

/// A port queue represents a single queue for a physical port, and should be used to send and receive data.
#[derive(Clone)]
pub struct PortQueue {
    // The Arc cost here should not affect anything, since we are really not doing anything to make it go in and out of
    // scope.
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
        self.port_id == other.port_id
            && self.txq == other.txq
            && self.rxq == other.rxq
            && self.port.is_native_kni() == other.port.is_native_kni()
    }
}

impl Eq for CacheAligned<PortQueue> {}

impl Hash for CacheAligned<PortQueue> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.port_id.hash(state);
        self.txq.hash(state);
        self.rxq.hash(state);
        self.port.is_native_kni().hash(state);
    }
}

#[derive(Clone)]
pub struct PortQueueTxBuffered {
    pub port_queue: PortQueue,
    tx_queue: Rc<RefCell<TxQueue>>,
}

struct TxQueue {
    ///tx queue for MBufs which could not be sent so far, organized as a VecDeque of MBuf batches
    tx_buffer: VecDeque<Vec<*mut MBuf>>,
    ///total no of MBufs in the queue
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
        let len = pkts.len();
        self.tx_buffer.push_back(pkts);
        self.tx_queue_len += len;
    }

    #[inline]
    fn push_front(&mut self, pkts: Vec<*mut MBuf>) {
        let len = pkts.len();
        self.tx_buffer.push_front(pkts);
        self.tx_queue_len += len;
    }

    #[inline]
    fn pop_front(&mut self) -> Option<Vec<*mut MBuf>> {
        let r = self.tx_buffer.pop_front();
        if r.is_some() {
            self.tx_queue_len -= r.as_ref().unwrap().len();
        }
        r
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
        self.batches() == 0
    }
}

/*  cannot use Drop, as we want to use Default when creating PmdPort
explicitly free PmdPorts if necessary
impl Drop for PmdPort {
    fn drop(&mut self) {
        if self.connected && self.should_close {
            unsafe {
                free_pmd_port(self.port);
            }
        }
    }
}
*/
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
    fn try_send(&mut self, pkts: &mut [*mut MBuf], to_send: u32) -> u32 {
        let sent = if self.port.is_native_kni() {
            unsafe { rte_kni_tx_burst(self.port.kni.unwrap().as_ptr(), pkts.as_mut_ptr(), to_send) }
        } else {
            if self.csum_offload() {
                let nb_prep = unsafe { eth_tx_prepare(self.port_id, self.txq, pkts.as_mut_ptr(), to_send as u16) };
                assert_eq!(nb_prep, to_send as u16);
            }
            unsafe { eth_tx_burst(self.port_id, self.txq, pkts.as_mut_ptr(), to_send as u16) as u32 }
        };
        let update = self.stats_tx.stats.load(Ordering::Relaxed) + sent as usize;
        self.stats_tx.stats.store(update, Ordering::Relaxed);
        sent
    }

    #[inline]
    fn send_queue(&mut self, pkts: &mut [*mut MBuf], to_send: u32) -> errors::Result<u32> {
        let sent = self.try_send(pkts, to_send);
        Ok(sent)
    }

    #[inline]
    fn recv_queue(&self, pkts: &mut [*mut MBuf], to_recv: u16) -> errors::Result<u32> {
        let start = unsafe { _rdtsc() };
        unsafe {
            let recv = if self.port.is_native_kni() {
                rte_kni_rx_burst(self.port.kni.unwrap().as_ptr(), pkts.as_mut_ptr(), to_recv as u32)
            } else {
                eth_rx_burst(self.port_id, self.rxq, pkts.as_mut_ptr(), to_recv)
            };
            //debug!("received { } packets", recv);
            let update = self.stats_rx.stats.load(Ordering::Relaxed) + recv as usize;
            self.stats_rx.stats.store(update, Ordering::Relaxed);

            if recv > 0 {
                let update = self.stats_rx.cycles.load(Ordering::Relaxed) + (_rdtsc() - start);
                self.stats_rx.cycles.store(update, Ordering::Relaxed);
            }
            Ok(recv)
        }
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
        self.port.n_tx_desc as u16
    }

    #[inline]
    pub fn n_rx_desc(&self) -> u16 {
        self.port.n_rx_desc as u16
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
    /// Send a batch of packets out this PortQueue. Note this method is internal to NetBricks (should not be directly
    /// called).
    #[inline]
    fn send(&mut self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        let len = pkts.len();
        self.send_queue(pkts, len as u32)
    }
}

impl PacketRx for PortQueue {
    /// Receive a batch of packets out this PortQueue. Note this method is internal to NetBricks (should not be directly
    /// called).
    #[inline]
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)> {
        let len = pkts.len() as u16;
        Ok((self.recv_queue(pkts, len)?, self.stats_rx.get_q_len() as i32))
    }

    #[inline]
    fn queued(&self) -> usize {
        let q_count = if self.port.is_physical() {
            unsafe { eth_rx_queue_count(self.port_id as u16, self.rxq as u16) }
        } else {
            1
        };
        if q_count < 0 {
            panic!(
                "eth_rx_queue_count failed for port_id= {} and rxq= {}",
                self.port_id, self.rxq
            );
            //q_count = 1;
        }
        self.stats_rx.set_q_len(q_count as usize);
        q_count as usize
    }
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

impl PortQueueTxBuffered {
    fn queue(&mut self, pkts: &mut [*mut MBuf]) {
        let len = pkts.len();
        let mut pkt_vec = Vec::with_capacity(len);
        pkt_vec.extend_from_slice(pkts);
        self.tx_queue.borrow_mut().push_back(pkt_vec);
        let update = self.port_queue.stats_tx.queued.load(Ordering::Relaxed) + len;
        self.port_queue.stats_tx.queued.store(update, Ordering::Relaxed);
        trace!("qlen= {}", self.tx_queue_len());
    }

    #[inline]
    fn tx_queue_len(&self) -> usize {
        RefCell::borrow(&self.tx_queue).len()
    }

    #[inline]
    fn tx_batches(&self) -> usize {
        RefCell::borrow(&self.tx_queue).batches()
    }

    #[inline]
    fn tx_queue_is_empty(&self) -> bool {
        RefCell::borrow(&self.tx_queue).is_empty()
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
                    self.port_queue.txq,
                    stamp,
                    sent,
                    to_send,
                    self.tx_queue_len(),
                    self.tx_batches()
                );
            }
            Ok(to_send)
        } else {
            loop {
                //let tx_q_len= self.tx_queue_len();
                let mut queued_batch = self.tx_queue.borrow_mut().pop_front().unwrap();
                let len = queued_batch.len();
                let sent = self.port_queue.try_send(&mut queued_batch[..], len as u32) as usize;
                trace!(
                    "txq={}, {}: sent {} of {} queued packets, tx q len = {}, batches= {}",
                    self.port_queue.txq,
                    stamp,
                    sent,
                    len,
                    self.tx_queue_len(),
                    self.tx_batches()
                );
                //assert!(sent <= tx_q_len);
                if sent < len {
                    let mut pkt_vec = Vec::with_capacity(len - sent);
                    pkt_vec.extend_from_slice(&queued_batch[sent..len]);
                    self.tx_queue.borrow_mut().push_front(pkt_vec);
                    self.queue(&mut pkts[0..to_send as usize]);
                    trace!(
                        "txq={}, {}: queuing full fresh {} packets, tx q len= {}, batches= {}",
                        self.port_queue.txq,
                        stamp,
                        to_send,
                        self.tx_queue_len(),
                        self.tx_batches()
                    );
                    break;
                }
                if self.tx_queue_is_empty() {
                    let sent = self.port_queue.try_send(pkts, to_send);
                    if sent < to_send {
                        self.queue(&mut pkts[sent as usize..to_send as usize]);
                        trace!(
                            "txq={}, {}: queuing remaining fresh {} packets, tx q len= {}, batches= {}",
                            self.port_queue.txq,
                            stamp,
                            to_send - sent,
                            self.tx_queue_len(),
                            self.tx_batches()
                        );
                    }
                    break;
                }
            }
            self.port_queue.stats_tx.set_q_len(self.tx_queue_len());
            Ok(to_send)
        }
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
    /// Send a batch of packets out this PortQueue. Note this method is internal to NetBricks (should not be directly
    /// called).
    #[inline]
    fn send(&mut self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        let len = pkts.len();
        self.send_queue(pkts, len as u32)
    }
}

impl PacketRx for PortQueueTxBuffered {
    /// Receive a batch of packets out this PortQueue. Note this method is internal to NetBricks (should not be directly
    /// called).
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.port_queue.fmt(f)
    }
}

impl PmdPort {
    #[inline]
    /// Determine the number of ports in a system.
    pub fn num_pmd_ports() -> i32 {
        unsafe { num_pmd_ports() }
    }

    /// Find a port ID given a PCI-E string.
    /*
    pub fn find_port_id(pcie: &str) -> i32 {
        let pcie_cstr = CString::new(pcie).unwrap();
        unsafe { find_port_with_pci_address(pcie_cstr.as_ptr()) }
    }
    */

    #[inline]
    pub fn port_id(&self) -> u16 {
        self.port
    }

    #[inline]
    pub fn name(&self) -> &String {
        &self.name
    }

    #[inline]
    pub fn kni_name(&self) -> Option<&String> {
        self.kni_name.as_ref()
    }

    #[inline]
    pub fn associated_dpdk_port_id(&self) -> Option<u16> {
        self.associated_dpdk_port_id
    }

    #[inline]
    pub fn linux_if(&self) -> Option<&String> {
        self.linux_if.as_ref()
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
        if self.net_spec.is_some() {
            let spec = self.net_spec.as_ref().unwrap();
            if spec.ip_net.is_some() {
                Some(spec.ip_net.unwrap().addr())
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    /// Number of configured RXQs.
    pub fn rxqs(&self) -> u16 {
        self.rxqs
    }

    #[inline]
    /// Number of configured TXQs.
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
        if self.fdir_conf.is_some() {
            u16::from_be(self.fdir_conf.unwrap().mask.dst_port_mask)
        } else {
            0x0000
        }
    }

    #[inline]
    pub fn get_ipv4_dst_mask(&self) -> u32 {
        if self.fdir_conf.is_some() {
            u32::from_be(self.fdir_conf.unwrap().mask.ipv4_mask.dst_ip)
        } else {
            0x00000000
        }
    }

    #[inline]
    pub fn is_native_kni(&self) -> bool {
        self.kni.is_some()
    }

    #[inline]
    pub fn is_virtio(&self) -> bool {
        *self.port_type() == PortType::Virtio
    }

    #[inline]
    pub fn is_physical(&self) -> bool {
        *self.port_type() == PortType::Physical
    }

    #[inline]
    pub fn get_rte_kni(&self) -> *mut RteKni {
        self.kni.unwrap().as_ptr()
    }

    pub fn new_queue_pair(port: &Arc<PmdPort>, rxq: u16, txq: u16) -> errors::Result<CacheAligned<PortQueue>> {
        if rxq > port.rxqs {
            Err(ErrorKind::BadRxQueue(port.port, rxq).into())
        } else if txq > port.txqs {
            Err(ErrorKind::BadTxQueue(port.port, txq).into())
        } else {
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
    }

    pub fn new_tx_buffered_queue_pair(
        port: &Arc<PmdPort>,
        rxq: u16,
        txq: u16,
    ) -> errors::Result<CacheAligned<PortQueueTxBuffered>> {
        if rxq > port.rxqs {
            Err(ErrorKind::BadRxQueue(port.port, rxq).into())
        } else if txq > port.txqs {
            Err(ErrorKind::BadTxQueue(port.port, txq).into())
        } else {
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
    }

    /// Get stats for an RX/TX queue pair.
    pub fn stats(&self, queue: u16) -> (usize, usize, usize) {
        let idx = queue as usize;
        (
            self.stats_rx[idx].stats.load(Ordering::Relaxed),
            self.stats_tx[idx].stats.load(Ordering::Relaxed),
            self.stats_rx[idx].get_max_q_len(),
        )
    }

    /// Get stats for an RX/TX queue pair.
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
            )
            .as_ref();

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
            "{0:>3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | {6: >20} |",
            "q", "rx_packets", "tx_packets", "tx_queued", "tx_q_len", "rx_q_len", "rx_cycles"
        );
        let (mut sin_p, mut sout_p, mut s_tx_queued) = (0usize, 0usize, 0usize);
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
        println!(
            "{0: >3} | {1: >20} | {2: >20} | {3: >20} | \n",
            "sum", sin_p, sout_p, s_tx_queued,
        );
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
        println!("");

        print!("   RX per queue offload capabilities: ");
        let rx_offload_capa = dev_info.rx_queue_offload_capa;
        for i in 0..64 {
            let offload_id = 1u64 << i;
            if offload_id & rx_offload_capa != 0 {
                let offload_capa_name = unsafe { CStr::from_ptr(rte_eth_dev_rx_offload_name(offload_id)) };
                print!("{} ", offload_capa_name.to_str().expect("bad string"));
            }
        }
        println!("");

        print!("   TX offload capabilities: ");
        let tx_offload_capa = dev_info.tx_offload_capa;
        for i in 0..64 {
            let offload_id = 1u64 << i;
            if offload_id & tx_offload_capa != 0 {
                let offload_capa_name = unsafe { CStr::from_ptr(rte_eth_dev_tx_offload_name(offload_id)) };
                print!("{} ", offload_capa_name.to_str().expect("bad string"));
            }
        }
        println!("");

        print!("   TX per queue offload capabilities: ");
        let tx_offload_capa = dev_info.tx_queue_offload_capa;
        for i in 0..64 {
            let offload_id = 1u64 << i;
            if offload_id & tx_offload_capa != 0 {
                let offload_capa_name = unsafe { CStr::from_ptr(rte_eth_dev_tx_offload_name(offload_id)) };
                print!("{} ", offload_capa_name.to_str().expect("bad string"));
            }
        }
        println!("");

        print!("   RSS offload capabilities: ");
        let rss_offload_capa = dev_info.flow_type_rss_offloads;
        for i in RTE_ETH_FLOW_UNKNOWN..RTE_ETH_FLOW_MAX {
            let offload_id = 1u64 << i;
            if offload_id & rss_offload_capa != 0 {
                let offload_capa_name = rss_flow_name(i as usize);
                print!("{} ", offload_capa_name);
            }
        }
        println!("");

        let x = (dev_info.max_rx_queues, dev_info.max_tx_queues);
        println!("   Max RX/TX queues:  {} / {}", x.0, x.1);

        let x = dev_info.max_mac_addrs;
        println!("   Max MAC addresses:  {}", x);
        println!("");
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
            let ret = unsafe {
                init_pmd_port(
                    port,
                    actual_rxqs as u16,
                    actual_txqs as u16,
                    rx_cores.as_ptr(),
                    tx_cores.as_ptr(),
                    nrxd,
                    ntxd,
                    loopbackv,
                    tsov,
                    csumoffloadv,
                    rx_mq_mode,
                    if fdir_conf.is_some() {
                        fdir_conf.unwrap() as *const RteFdirConf
                    } else {
                        ptr::null()
                    },
                )
            };
            if ret == 0 {
                Ok(Arc::new(PmdPort {
                    name: name.to_string(),
                    kni_name: kni,
                    port_type,
                    port,
                    kni: None,
                    linux_if,
                    rxqs: actual_rxqs as u16,
                    txqs: actual_txqs as u16,
                    rx_cores: Some(rx_cores.to_vec()),
                    tx_cores: Some(tx_cores.to_vec()),
                    n_rx_desc: nrxd,
                    n_tx_desc: ntxd,
                    csumoffload,
                    driver,
                    stats_rx: (0..actual_rxqs).map(|_| Arc::new(PortStats::new())).collect(),
                    stats_tx: (0..actual_txqs).map(|_| Arc::new(PortStats::new())).collect(),
                    fdir_conf: if fdir_conf.is_some() {
                        Some(fdir_conf.unwrap().clone())
                    } else {
                        None
                    },
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

    fn new_kni_port(
        name: &str,
        kni_port_params: Box<KniPortParams>,
        rx_cores: &[i32],
        tx_cores: &[i32],
        net_spec: Option<NetSpec>,
    ) -> errors::Result<Arc<PmdPort>> {
        let associated_dpdk_port_id = kni_port_params.associated_dpdk_port_id;
        let p_kni_port_params: *mut KniPortParams = Box::into_raw(kni_port_params);
        unsafe {
            // This call returns a pointer to an opaque C struct
            let p_kni = kni_alloc(associated_dpdk_port_id, p_kni_port_params);
            if !p_kni.is_null() {
                Ok(Arc::new(PmdPort {
                    name: name.to_string(),
                    kni_name: None, // kni ports do not have an associated kni
                    port_type: PortType::Kni,
                    port: associated_dpdk_port_id,
                    kni: Some(Unique::new(p_kni).unwrap()),
                    linux_if: kni_get_name(p_kni),
                    rx_cores: Some(rx_cores.to_vec()),
                    tx_cores: Some(tx_cores.to_vec()),
                    stats_rx: (0..rx_cores.len()).map(|_| Arc::new(PortStats::new())).collect(),
                    stats_tx: (0..tx_cores.len()).map(|_| Arc::new(PortStats::new())).collect(),
                    rxqs: rx_cores.len() as u16,
                    txqs: tx_cores.len() as u16,
                    net_spec,
                    associated_dpdk_port_id: Some(associated_dpdk_port_id),
                    ..Default::default()
                }))
            } else {
                Err(ErrorKind::FailedToInitializeKni(name.to_string()).into())
            }
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
        fdir_conf: Option<&RteFdirConf>,
        flow_steering_mode: Option<FlowSteeringMode>,
        net_spec: Option<NetSpec>,
        associated_dpdk_port_id: Option<u16>,
    ) -> errors::Result<Arc<PmdPort>> {
        let cannonical_spec = PmdPort::cannonicalize_pci(spec);
        debug!("attach_pmd_device, port = {:?}", cannonical_spec);
        let mut ports: Vec<u16> = Vec::with_capacity(16);
        let rc = unsafe { attach_device((cannonical_spec[..]).as_ptr(), ports.as_mut_ptr(), 16) };
        if rc >= 0 {
            unsafe {
                ports.set_len(rc as usize);
            }
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
                port as u16,
                rx_cores,
                tx_cores,
                nrxd,
                ntxd,
                loopback,
                tso,
                csumoffload,
                driver,
                port_type,
                fdir_conf,
                flow_steering_mode,
                net_spec,
                associated_dpdk_port_id,
            )
        } else {
            Err(ErrorKind::BadDev(String::from(spec)).into())
        }
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
                    fdir_conf,
                    port_config.flow_steering,
                    port_config.net_spec.clone(),
                    associated_port.map_or(None, |p| Some(p.port_id())),
                )
            }
            "kni" => {
                if associated_port.is_none() {
                    warn!("kni port {} has no associated dpdk port", name);
                    Err(ErrorKind::FailedToInitializeKni(name.to_string()).into())
                } else {
                    let port_id = associated_port.unwrap().port_id();
                    let rx_cores = associated_port.map_or(rx_cores, |p| &p.rx_cores.as_ref().unwrap()[..]);
                    let tx_cores = associated_port.map_or(tx_cores, |p| &p.tx_cores.as_ref().unwrap()[..]);

                    PmdPort::new_kni_port(
                        name,
                        Box::new(KniPortParams::new(
                            port_id,
                            rx_cores[0] as u32,
                            tx_cores[0] as u32,
                            &port_config.k_cores,
                        )),
                        rx_cores,
                        tx_cores,
                        port_config.net_spec.clone(),
                    )
                }
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
            fdir_conf: None,
            flow_steering: None,
            kni: None,
            driver: DriverType::Unknown,
            net_spec: None,
        };
        PmdPort::new_port_from_configuration(&config, None)
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
        let mut address: rte_ether_addr = rte_ether_addr { addr_bytes: [0u8; 6] };
        unsafe {
            rte_eth_macaddr_get(self.port, &mut address);
        }
        MacAddress::new(address.addr_bytes)
    }
}
