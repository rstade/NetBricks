use super::MBuf;
use headers::MacAddress;
use std::os::raw::{ c_char, c_void };
use std::ptr;

pub enum RteKni {}

pub enum RteFlow {}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum RteLogLevel {
    RteLogEmerg = 1,
    RteLogAlert = 2,
    RteLogCrit = 3,
    RteLogErr = 4,
    RteLogWarning = 5,
    RteLogNotice = 6,
    RteLogInfo = 7,
    RteLogDebug = 8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum RteLogtype {
	RteLogtypeEal = 0,
	RteLogtypeMalloc = 1,
	RteLogtypeRing = 2,		
	RteLogtypeMempool = 3,
	RteLogtypeTimer = 4,
	RteLogtypePmd = 5,		
	RteLogtypeHash = 6,
	RteLogtypeLpm = 7,
	RteLogtypeKni = 8,		
	RteLogtypeAcl = 9,
	RteLogtypePower = 10,
	RteLogtypeMeter = 11,		
	RteLogtypeSched = 12,
	RteLogtypePort = 13,
	RteLogtypeTable = 14,		
	RteLogtypePipeline = 15,
	RteLogtypeMbuf = 16,
	RteLogtypeCryptodef = 17,		
	RteLogtypeEfd = 18,
	RteLogtypeEventdev = 19,
	RteLogtypeGso = 20,		
	
}
/* see kni.c
struct kni_port_params {
        uint16_t port_id; // Port ID 
        unsigned lcore_rx; // lcore ID for RX 
        unsigned lcore_tx; // lcore ID for TX
        uint32_t nb_lcore_k; // Number of lcores for KNI multi kernel threads 
        uint32_t nb_kni; // Number of KNI devices to be created 
        unsigned lcore_k[KNI_MAX_KTHREAD]; // lcore ID list for kthreads 
        struct rte_kni *kni[KNI_MAX_KTHREAD]; // KNI context pointers 
} __rte_cache_aligned;
*/
pub const KNI_MAX_KTHREAD: usize = 32;

#[repr(C)]
pub struct KniPortParams {
    pub port_id: u16,
    // Port ID
    pub lcore_rx: u32,
    // lcore ID for RX
    pub lcore_tx: u32,
    // lcore ID for TX
    pub nb_lcore_k: u32,
    // Number of lcores for KNI multi kernel threads
    pub nb_kni: u32,
    // Number of KNI devices to be created
    pub lcore_k: [u32; KNI_MAX_KTHREAD],
    // lcore ID list for kthreads
    pub kni: [*mut RteKni; KNI_MAX_KTHREAD], // KNI context pointers
}

impl KniPortParams {
    pub fn new(port_id: u16, lcore_rx: u32, lcore_tx: u32, lcore_k: &Vec<i32>) -> KniPortParams {
        let mut params = KniPortParams {
            port_id: port_id, // Port ID
            lcore_rx: lcore_rx, // lcore ID for RX
            lcore_tx: lcore_tx, // lcore ID for TX
            nb_lcore_k: lcore_k.len() as u32, // Number of lcores for KNI multi kernel threads
            nb_kni: 1,
            lcore_k: [0u32; KNI_MAX_KTHREAD], // lcore ID list for kthreads
            kni: [ptr::null_mut(); KNI_MAX_KTHREAD], // KNI context pointers
        };
        for i in 0..lcore_k.len() {
            params.lcore_k[i] = lcore_k[i] as u32;
        }
        params
    }
}

#[repr(C)]
pub struct RteFlowError {
    pub err_type: u32,
    pub cause: *mut c_void,
    pub message: *mut c_char,
}

#[link(name = "zcsi")]
extern "C" {
    pub fn init_system_whitelisted(
        name: *const c_char,
        nlen: i32,
        lcore_mask: u64,
        core: i32,
        whitelist: *mut *const c_char,
        wlcount: i32,
        pool_size: u32,
        cache_size: u32,
        slots: u16,
        vdevs: *mut *const c_char,
        vdev_count: i32,
    ) -> i32;
    pub fn init_thread(tid: i32, core: i32) -> i32;
    pub fn init_secondary(
        name: *const c_char,
        nlen: i32,
        lcore_mask: u64,
        core: i32,
        vdevs: *mut *const c_char,
        vdev_count: i32,
    ) -> i32;
    pub fn init_pmd_port(
        port: i32,
        rxqs: i32,
        txqs: i32,
        rx_cores: *const i32,
        tx_cores: *const i32,
        nrxd: i32,
        ntxd: i32,
        loopback: i32,
        tso: i32,
        csumoffload: i32,
    ) -> i32;
    pub fn free_pmd_port(port: i32) -> i32;
    pub fn eth_rx_burst(port: i32, qid: i32, pkts: *mut *mut MBuf, len: u16) -> u32; // sta
    //rte_eth_tx_burst is inline C, we cannot directly use it here:
    pub fn eth_tx_burst(port: i32, qid: i32, pkts: *mut *mut MBuf, len: u16) -> u32;
    pub fn num_pmd_ports() -> i32;
    pub fn rte_eth_macaddr_get(port: i32, address: *mut MacAddress);
    pub fn init_bess_eth_ring(ifname: *const c_char, core: i32) -> i32;
    pub fn init_ovs_eth_ring(iface: i32, core: i32) -> i32;
    pub fn find_port_with_pci_address(pciaddr: *const c_char) -> i32;
    pub fn attach_pmd_device(dev: *const c_char) -> i32;
    // FIXME: Generic PMD info
    pub fn max_rxqs(port: i32) -> i32;
    pub fn max_txqs(port: i32) -> i32;
    pub fn mbuf_alloc() -> *mut MBuf;
    pub fn mbuf_free(buf: *mut MBuf);
    pub fn mbuf_alloc_bulk(array: *mut *mut MBuf, cnt: u32) -> i32;
    pub fn mbuf_free_bulk(array: *mut *mut MBuf, cnt: i32) -> i32;
    pub fn crc_hash_native(to_hash: *const u8, size: u32, iv: u32) -> u32;
    pub fn ipv4_cksum(payload: *const u8) -> u16;

    //usually called already by rte_eal_init when e.g. --vdev netkni0:
    pub fn rte_kni_init(max_kni_ifaces: u32);
    pub fn kni_alloc(port_id: u16, kni_port_params: *mut KniPortParams) -> *mut RteKni; // sta
    pub fn rte_kni_release(kni: *mut RteKni) -> i32; //sta
    pub fn rte_kni_handle_request(kni: *mut RteKni) -> i32; //sta
    pub fn rte_kni_rx_burst(kni: *mut RteKni, pkts: *mut *mut MBuf, len: u32) -> u32; //sta
    pub fn rte_kni_tx_burst(kni: *mut RteKni, pkts: *mut *mut MBuf, len: u32) -> u32; //sta

    pub fn rte_log_set_global_level(level: RteLogLevel);
    pub fn rte_log_get_global_level() -> u32;
    pub fn rte_log_set_level(logtype: RteLogtype, level: RteLogLevel) -> i32;
    pub fn rte_log_get_level(logtype: RteLogtype) -> i32;    

    pub fn generate_tcp_flow(port_id: u16, rx_q: u16,
                             src_ip: u32, src_mask: u32,
                             dest_ip: u32, dest_mask: u32,
                             src_port: u16, src_port_mask: u16,
                             dst_port: u16, dst_port_mask: u16,
                             error: *const RteFlowError) -> *mut RteFlow;
}
