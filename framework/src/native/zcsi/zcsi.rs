use super::MBuf;
use headers::MacAddress;
use std::os::raw::c_char;

#[allow(non_camel_case_types)]
pub enum Struct_rte_kni { }

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
    pub fn eth_tx_burst(port: i32, qid: i32, pkts: *mut *mut MBuf, len: u16) -> u32; //sta, rte_eth_tx_burst is inline C, we cannot directly use it here
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

    pub fn rte_kni_init(max_kni_ifaces: u32); //sta, usually called already by rte_eal_init when e.g. --vdev netkni0
    pub fn kni_alloc(port_id: u8) -> *mut Struct_rte_kni; // sta
    pub fn rte_kni_release(kni: *mut Struct_rte_kni) -> i32; //sta
    pub fn rte_kni_handle_request(kni: *mut Struct_rte_kni) -> i32; //sta
    pub fn rte_kni_rx_burst(kni: *mut Struct_rte_kni, pkts: *mut *mut MBuf, len: u32) -> u32; //sta
    pub fn rte_kni_tx_burst(kni: *mut Struct_rte_kni, pkts: *mut *mut MBuf, len: u32) -> u32; //sta

}
