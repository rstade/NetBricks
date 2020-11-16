use native::zcsi::rte_ethdev_api::{
    rte_eth_desc_lim, rte_eth_dev_info, rte_eth_dev_portconf, rte_eth_rxconf, rte_eth_rxseg, rte_eth_rxseg_capa,
    rte_eth_switch_info, rte_eth_thresh, rte_eth_txconf, RTE_ETH_FLOW_MAX,
};

const RSS_FLOW_NAMES: [&str; (RTE_ETH_FLOW_MAX + 1) as usize] = [
    "Unknown",
    "Raw",
    "Ipv4",
    "FragIpv4",
    "NonFragIpv4Tcp",
    "NonFragIpv4Udp",
    "NonFragIpv4Sctp",
    "NonFragIpv4Other",
    "IPv6",
    "FragIpv6",
    "NonFragIpv6Tcp",
    "NonFragIpv6Udp",
    "NonFragIpv6Sctp",
    "NonFragIpv6Other",
    "L2Payload",
    "Ipv6Ex",
    "Ipv6TcpEx",
    "Ipv6UdpEx",
    "Port",
    "Vxlan",
    "Geneve",
    "Nvgre",
    "VxlanGpe",
    "GTPU",
    "Max",
];

pub fn rss_flow_name(rss_flow_id: usize) -> &'static str {
    if rss_flow_id <= RTE_ETH_FLOW_MAX as usize {
        RSS_FLOW_NAMES[rss_flow_id]
    } else {
        RSS_FLOW_NAMES[0]
    }
}

use std::os::raw::c_void;
use std::ptr;

impl rte_eth_dev_info {
    pub fn new_null() -> rte_eth_dev_info {
        rte_eth_dev_info {
            device: ptr::null_mut(),
            driver_name: ptr::null(),
            if_index: 0,
            min_mtu: 0,
            max_mtu: 0,
            dev_flags: ptr::null(),
            min_rx_bufsize: 0,
            max_rx_pktlen: 0,
            max_lro_pkt_size: 0,
            max_rx_queues: 0,
            max_tx_queues: 0,
            max_mac_addrs: 0,
            max_hash_mac_addrs: 0,
            max_vfs: 0,
            max_vmdq_pools: 0,
            rx_seg_capa: rte_eth_rxseg_capa {
                _bitfield_1: Default::default(),
                max_nseg: 0,
                reserved: 0,
            },
            rx_offload_capa: 0,
            tx_offload_capa: 0,
            rx_queue_offload_capa: 0,
            tx_queue_offload_capa: 0,
            reta_size: 0,
            hash_key_size: 0,
            flow_type_rss_offloads: 0,
            default_rxconf: rte_eth_rxconf::new_null(),
            default_txconf: rte_eth_txconf::new_null(),
            vmdq_queue_base: 0,
            vmdq_queue_num: 0,
            vmdq_pool_base: 0,
            rx_desc_lim: rte_eth_desc_lim::new_null(),
            tx_desc_lim: rte_eth_desc_lim::new_null(),
            speed_capa: 0,
            nb_rx_queues: 0,
            nb_tx_queues: 0,
            default_rxportconf: rte_eth_dev_portconf::new_null(),
            default_txportconf: rte_eth_dev_portconf::new_null(),
            dev_capa: 0,
            switch_info: rte_eth_switch_info::new_null(),
            reserved_64s: [0u64, 0u64],
            reserved_ptrs: [0u64 as *mut ::std::os::raw::c_void; 2],
        }
    }
}

impl rte_eth_rxconf {
    pub fn new_null() -> rte_eth_rxconf {
        rte_eth_rxconf {
            rx_thresh: rte_eth_thresh::new_null(),
            rx_free_thresh: 0,
            rx_drop_en: 0,
            rx_deferred_start: 0,
            rx_nseg: 0,
            offloads: 0,
            rx_seg: 0u64 as *mut rte_eth_rxseg,
            reserved_64s: [0u64; 2],
            reserved_ptrs: [0u64 as *mut c_void; 2],
        }
    }
}

impl rte_eth_txconf {
    pub fn new_null() -> rte_eth_txconf {
        rte_eth_txconf {
            tx_thresh: rte_eth_thresh::new_null(),
            tx_rs_thresh: 0,
            tx_free_thresh: 0,
            tx_deferred_start: 0,
            offloads: 0,
            reserved_64s: [0u64; 2],
            reserved_ptrs: [0u64 as *mut c_void; 2],
        }
    }
}

impl rte_eth_thresh {
    pub fn new_null() -> rte_eth_thresh {
        rte_eth_thresh {
            pthresh: 0,
            hthresh: 0,
            wthresh: 0,
        }
    }
}

impl rte_eth_desc_lim {
    pub fn new_null() -> rte_eth_desc_lim {
        rte_eth_desc_lim {
            nb_max: 0,
            nb_min: 0,
            nb_align: 0,
            nb_seg_max: 0,
            nb_mtu_seg_max: 0,
        }
    }
}

impl rte_eth_dev_portconf {
    pub fn new_null() -> rte_eth_dev_portconf {
        rte_eth_dev_portconf {
            burst_size: 0,
            ring_size: 0,
            nb_queues: 0,
        }
    }
}

impl rte_eth_switch_info {
    pub fn new_null() -> rte_eth_switch_info {
        rte_eth_switch_info {
            name: ptr::null(),
            domain_id: 0,
            port_id: 0,
        }
    }
}
