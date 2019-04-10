use super::super::super::headers::IpHeader;
use super::ethdev::rte_eth_dev_info;
use super::MBuf;
use eui48::MacAddress;
use std::convert;
use std::ffi::CStr;
use std::fmt;
use std::io;
use std::net::Ipv4Addr;
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::str::Utf8Error;

pub enum RteKni {}
pub enum RteFlow {}

/*
 * A packet can be identified by hardware as different flow types. Different
 * NIC hardwares may support different flow types.
 * Basically, the NIC hardware identifies the flow type as deep protocol as
 * possible, and exclusively. For example, if a packet is identified as
 * 'RTE_ETH_FLOW_NONFRAG_IPV4_TCP', it will not be any of other flow types,
 * though it is an actual IPV4 packet.
 * Note that the flow types are used to define RSS offload types in
 * rte_ethdev.h.
 *
#define RTE_ETH_FLOW_UNKNOWN             0
#define RTE_ETH_FLOW_RAW                 1
#define RTE_ETH_FLOW_IPV4                2
#define RTE_ETH_FLOW_FRAG_IPV4           3
#define RTE_ETH_FLOW_NONFRAG_IPV4_TCP    4
#define RTE_ETH_FLOW_NONFRAG_IPV4_UDP    5
#define RTE_ETH_FLOW_NONFRAG_IPV4_SCTP   6
#define RTE_ETH_FLOW_NONFRAG_IPV4_OTHER  7
#define RTE_ETH_FLOW_IPV6                8
#define RTE_ETH_FLOW_FRAG_IPV6           9
#define RTE_ETH_FLOW_NONFRAG_IPV6_TCP   10
#define RTE_ETH_FLOW_NONFRAG_IPV6_UDP   11
#define RTE_ETH_FLOW_NONFRAG_IPV6_SCTP  12
#define RTE_ETH_FLOW_NONFRAG_IPV6_OTHER 13
#define RTE_ETH_FLOW_L2_PAYLOAD         14
#define RTE_ETH_FLOW_IPV6_EX            15
#define RTE_ETH_FLOW_IPV6_TCP_EX        16
#define RTE_ETH_FLOW_IPV6_UDP_EX        17
#define RTE_ETH_FLOW_PORT               18
/**< Consider device port number as a flow differentiator */
#define RTE_ETH_FLOW_VXLAN              19 /**< VXLAN protocol based flow */
#define RTE_ETH_FLOW_GENEVE             20 /**< GENEVE protocol based flow */
#define RTE_ETH_FLOW_NVGRE              21 /**< NVGRE protocol based flow */
#define RTE_ETH_FLOW_MAX                22
*/

pub const RTE_ETH_FLOW_NONFRAG_IPV4_TCP: u16 = 4;

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

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum RteFilterType {
    RteEthFilterNone = 0,
    RteEthFilterMacvlan = 1,
    RteEthFilterEthertype = 2,
    RteEthFilterFlexible = 3,
    RteEthFilterSyn = 4,
    RteEthFilterNtuple = 5,
    RteEthFilterTunnel = 6,
    RteEthFilterFdir = 7,
    RteEthFilterHash = 8,
    RteEthFilterL2Tunnel = 9,
    RteEthFilterGeneric = 10,
    RteEthFilterMax = 11,
}

impl convert::From<i32> for RteFilterType {
    fn from(i: i32) -> RteFilterType {
        match i {
            0 => RteFilterType::RteEthFilterNone,
            1 => RteFilterType::RteEthFilterMacvlan,
            2 => RteFilterType::RteEthFilterEthertype,
            3 => RteFilterType::RteEthFilterFlexible,
            4 => RteFilterType::RteEthFilterSyn,
            5 => RteFilterType::RteEthFilterNtuple,
            6 => RteFilterType::RteEthFilterTunnel,
            7 => RteFilterType::RteEthFilterFdir,
            8 => RteFilterType::RteEthFilterHash,
            9 => RteFilterType::RteEthFilterL2Tunnel,
            10 => RteFilterType::RteEthFilterGeneric,
            11 => RteFilterType::RteEthFilterMax,
            _ => RteFilterType::RteEthFilterNone,
        }
    }
}

impl fmt::Display for RteFilterType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", &self)
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum RteFilterOp {
    /** used to check whether the type filter is supported */
    RteEthFilterNop = 0,
    /**< add filter entry */
    RteEthFilterAdd = 1,
    /**< update filter entry */
    RteEthFilterUpdate = 2,
    /**< delete filter entry */
    RteEthFilterDelete = 3,
    /**< flush all entries */
    RteEthFilterFlush = 4,
    /**< get filter entry */
    RteEthFilterGet = 5,
    /**< configurations */
    RteEthFilterSet = 6,
    /**< retrieve information */
    RteEthFilterInfo = 7,
    /**< retrieve statistics */
    RteEthFilterStats = 8,
    RteEthFilterOpMax,
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
            port_id,                          // Port ID
            lcore_rx,                         // lcore ID for RX
            lcore_tx,                         // lcore ID for TX
            nb_lcore_k: lcore_k.len() as u32, // Number of lcores for KNI multi kernel threads
            nb_kni: 1,
            lcore_k: [0u32; KNI_MAX_KTHREAD],        // lcore ID list for kthreads
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

pub unsafe fn kni_get_name(p_kni: *const RteKni) -> Option<String> {
    let kni_if_raw: *const c_char = rte_kni_get_name(p_kni);
    let slice = CStr::from_ptr(kni_if_raw).to_str();
    match slice {
        Ok(slice) => Some(String::from(slice)),
        Err(_) => None,
    }
}

const RTE_ETHDEV_QUEUE_STAT_CNTRS: usize = 16;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RteEthStats {
    pub ipackets: u64,
    pub opackets: u64,
    pub ibytes: u64,
    pub obytes: u64,
    pub imissed: u64,
    pub ierrors: u64,
    pub oerrors: u64,
    pub rx_nombuf: u64,
    pub q_ipackets: [u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
    pub q_opackets: [u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
    pub q_ibytes: [u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
    pub q_obytes: [u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
    pub q_errors: [u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
}

impl RteEthStats {
    pub fn new() -> RteEthStats {
        RteEthStats {
            ipackets: 0u64,
            opackets: 0u64,
            ibytes: 0u64,
            obytes: 0u64,
            imissed: 0u64,
            ierrors: 0u64,
            oerrors: 0u64,
            rx_nombuf: 0u64,
            q_ipackets: [0u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
            q_opackets: [0u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
            q_ibytes: [0u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
            q_obytes: [0u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
            q_errors: [0u64; RTE_ETHDEV_QUEUE_STAT_CNTRS],
        }
    }
}

impl fmt::Display for RteEthStats {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "imissed= {}, rx_no_mbuf= {}\n", self.imissed, self.rx_nombuf).unwrap();
        write!(
            f,
            "{0:>3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | {6: >20}\n",
            "q", "ipackets", "opackets", "ibytes", "obytes", "ierrors", "oerrors"
        )
        .unwrap();
        for q in 0..4 {
            write!(
                f,
                "{0:>3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | \n",
                q, self.q_ipackets[q], self.q_opackets[q], self.q_ibytes[q], self.q_obytes[q], self.q_errors[q],
            )
            .unwrap();
        }
        write!(
            f,
            "{0:>3} | {1: >20} | {2: >20} | {3: >20} | {4: >20} | {5: >20} | {6: >20}\n",
            "sum", self.ipackets, self.opackets, self.ibytes, self.obytes, self.ierrors, self.oerrors,
        )
        .unwrap();
        Ok(())
    }
}

pub const RTE_ETH_XSTATS_NAME_SIZE: usize = 64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RteEthXstatName {
    pub name: [c_char; RTE_ETH_XSTATS_NAME_SIZE],
}

impl RteEthXstatName {
    pub fn to_str(&self) -> Result<&str, Utf8Error> {
        unsafe { CStr::from_ptr(&self.name[0] as *const c_char).to_str() }
    }
    pub fn to_ptr(&self) -> *const c_char {
        &self.name[0]
    }
}

/**
 * A structure used to define the input for IPV4 flow
 */
#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub struct RteEthIpv4Flow {
    pub src_ip: u32, // < IPv4 source address in big endian.
    pub dst_ip: u32, // < IPv4 destination address in big endian.
    pub tos: u8,     // < Type of service to match.
    pub ttl: u8,     // < Time to live to match.
    pub proto: u8,   // < Protocol, next header in big endian.
}

impl fmt::Display for RteEthIpv4Flow {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(src_ip= {}, dst_ip= {}, tos= {}, ttl= {}, proto= {})",
            Ipv4Addr::from(self.src_ip),
            Ipv4Addr::from(self.dst_ip),
            self.tos,
            self.ttl,
            self.proto
        )
    }
}
/**
* A structure used to define the input for IPV6 flow
*/
#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub struct RteEthIpv6Flow {
    pub src_ip: [u32; 4], // IPv6 source address in big endian.
    pub dst_ip: [u32; 4], // IPv6 destination address in big endian.
    pub tc: u8,           // Traffic class to match.
    pub proto: u8,        // Protocol, next header to match.
    pub hop_limits: u8,   // Hop limits to match.
}

/**
 * A structure used to define the input for IPV4 UDP flow
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RteEthUdpv4Flow {
    pub ip: RteEthIpv4Flow, // < IPv4 fields to match.
    pub src_port: u16,      // < UDP source port in big endian.
    pub dst_port: u16,      // < UDP destination port in big endian.
}

/**
 * A structure used to define the input for IPV4 TCP flow
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RteEthTcpv4Flow {
    pub ip: RteEthIpv4Flow, // < IPv4 fields to match.
    pub src_port: u16,      // < TCP source port in big endian.
    pub dst_port: u16,      // < TCP destination port in big endian.
    pub _padding: [u8; 28],
}

/**
 * A structure used to contain extend input of flow
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RteEthFdirFlowExt {
    pub vlan_tci: u16,
    /**< It is filled by the flexible payload to match. */
    pub flexbytes: [u8; 16],
    pub is_vf: u8,   // 1 for VF, 0 for port dev
    pub dst_id: u16, // VF ID, available when is_vf is 1
}

/*
 * An union contains the inputs for all types of flow
 * Items in flows need to be in big endian

union rte_eth_fdir_flow {
    struct rte_eth_l2_flow     l2_flow;
    struct rte_eth_udpv4_flow  udp4_flow;
    struct rte_eth_tcpv4_flow  tcp4_flow;
    struct rte_eth_sctpv4_flow sctp4_flow;
    struct rte_eth_ipv4_flow   ip4_flow;
    struct rte_eth_udpv6_flow  udp6_flow;
    struct rte_eth_tcpv6_flow  tcp6_flow;
    struct rte_eth_sctpv6_flow sctp6_flow;  // largest struct: 43 bytes
    struct rte_eth_ipv6_flow   ipv6_flow;
    struct rte_eth_mac_vlan_flow mac_vlan_flow;
    struct rte_eth_tunnel_flow   tunnel_flow;
};
*/

/**
 * A structure used to define the input for a flow director filter entry
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RteEthFdirInputTcpv4 {
    pub flow_type: u16, // e.g. RTE_ETH_FLOW_NONFRAG_IPV4_TCP
    pub flow: RteEthTcpv4Flow,
    // < Flow fields to match, dependent on flow_type */
    pub flow_ext: RteEthFdirFlowExt,
    // < Additional fields to match */
}

/**
 * Behavior will be taken if FDIR match
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub enum RteEthFdirBehavior {
    RteEthFdirAccept = 0,
    RteEthFdirReject = 1,
    RteEthFdirPassthru = 2,
}

/**
 * Flow director report status
 * It defines what will be reported if FDIR entry is matched.
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub enum RteEthFdirStatus {
    RteEthFdirNoReportStatus = 0, // < Report nothing.
    RteEthFdirReportId = 1,       // < Only report FD ID.
    RteEthFdirReportIdFlex4 = 2,  // < Report FD ID and 4 flex bytes.
    RteEthFdirReportFlex8 = 3,    // < Report 8 flex bytes.
}

/**
 * A structure used to define an action when match FDIR packet filter.
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RteEthFdirAction {
    pub rx_queue: u16,                   // < Queue assigned to if FDIR match.
    pub behavior: RteEthFdirBehavior,    // < Behavior will be taken
    pub report_status: RteEthFdirStatus, // < Status report option
    pub flex_off: u8,
    //   If report_status is RteEthFdirReportIdFlex4 or
    //   RteEthFdirReportFlex8, flex_off specifies where the reported
    //   flex bytes start from in flexible payload.
}

/**
 * A structure used to define the flow director filter entry by filter_ctrl API
 * It supports RTE_ETH_FILTER_FDIR with RTE_ETH_FILTER_ADD and
 * RTE_ETH_FILTER_DELETE operations.
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub struct RteEthFdirFilter {
    pub soft_id: u32,
    /**< ID, an unique value is required when deal with FDIR entry */
    pub input: RteEthFdirInputTcpv4, // < Input set
    pub action: RteEthFdirAction, // < Action taken when match
}

pub const RTE_ETH_INSET_SIZE_MAX: usize = 128;
/**< Max length of input set. */

/**
 * Input set fields for Flow Director and Hash filters
 */
#[derive(Clone, Copy)]
#[repr(C)]
pub enum RteEthInputSetField {
    Unknown = 0,

    /* L2 */
    L2SrcMac = 1,
    L2DstMac,
    L2OuterVlan,
    L2InnerVlan,
    L2EtherType,

    /* L3 */
    L3SrcIp4 = 129,
    L3DstIp4,
    L3SrcIp6,
    L3DstIp6,
    L3Ip4Tos,
    L3Ip4Proto,
    L3Ip6Tc,
    L3Ip6NextHeader,
    L3Ip4Ttl,
    L3Ip6HopLimits,

    /* L4 */
    L4UdpSrcPort = 257,
    L4UdpDstPort,
    L4TcpSrcPort,
    L4TcpDstPort,
    L4SctpSrcPort,
    L4SctpDstPort,
    L4SctpVerificationTag,

    /* Tunnel */
    TunnelL2InnerDstMac = 385,
    TunnelL2InnerSrcMac,
    TunnelL2InnerVlan,
    TunnelL4UdpKey,
    TunnelGreKey,

    /* Flexible Payload */
    FlexPayload1stWord = 641,
    FlexPayload2ndWord,
    FlexPayload3rdWord,
    FlexPayload4thWord,
    FlexPayload5thWord,
    FlexPayload6thWord,
    FlexPayload7thWord,
    FlexPayload8thWord,

    DEFAULT = 65533,
    NONE = 65534,
    MAX = 65535,
}

/**
 * Filters input set operations
 */
#[repr(C)]
pub enum RteFilterInputSetOp {
    OpUnknown,
    Select,
    /**< select input set */
    Add,
    /**< add input set entry */
    OpMax,
}

/**
 * A structure used to define the input set configuration for
 * flow director and hash filters
 */
#[repr(C)]
pub struct RteEthInputSetConf {
    pub flow_type: u16,
    pub inset_size: u16,
    pub field: [RteEthInputSetField; RTE_ETH_INSET_SIZE_MAX],
    pub op: RteFilterInputSetOp,
}

/**
 * Flow Director filter information types.
 */
#[repr(C)]
pub enum RteEthFdirFilterInfoType {
    RteEthFdirFilterInfoTypeUnknown = 0,
    /** Flow Director filter input set configuration */
    RteEthFdirFilterInputSetSelect = 1,
    RteEthFdirFilterInfoTypeMax = 2,
}

/**
 * A structure used to set FDIR filter information, to support filter type
 * of 'RTE_ETH_FILTER_FDIR' RTE_ETH_FDIR_FILTER_INPUT_SET_SELECT operation.
 */
#[repr(C)]
pub struct RteEthFdirFilterInfo {
    pub info_type: RteEthFdirFilterInfoType,
    /**< Information type */
    /** Details of fdir filter information */
    /** Flow Director input set configuration per port */
    pub input_set_conf: RteEthInputSetConf,
}

/**
 *  A structure used to configure FDIR masks that are used by the device
 *  to match the various fields of RX packet headers.
 */
#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub struct RteEthFdirMasks {
    pub vlan_tci_mask: u16, // < Bit mask for vlan_tci in big endian
    /** Bit mask for ipv4 flow in big endian. */
    pub ipv4_mask: RteEthIpv4Flow,
    /** Bit maks for ipv6 flow in big endian. */
    pub ipv6_mask: RteEthIpv6Flow,
    /** Bit mask for L4 source port in big endian. */
    pub src_port_mask: u16,
    /** Bit mask for L4 destination port in big endian. */
    pub dst_port_mask: u16,
    /** 6 bit mask for proper 6 bytes of Mac address, bit 0 matches the
    first byte on the wire */
    pub mac_addr_byte_mask: u8,
    /** Bit mask for tunnel ID in big endian. */
    pub tunnel_id_mask: u32,
    pub tunnel_type_mask: u8, // < 1 - Match tunnel type,  0 - Ignore tunnel type.
}

/**
 *  Flow Director setting modes: none, signature or perfect.
 */
#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub enum RteFdirMode {
    RteFdirModeNone = 0,           // Disable FDIR support.
    RteFdirModeSignature = 1,      // Enable FDIR signature filter mode.
    RteFdirModePerfect = 2,        // Enable FDIR perfect filter mode.
    RteFdirModePerfectMacVlan = 3, // Enable FDIR filter mode - MAC VLAN.
    RteFdirModePerfectTunnel = 4,  // Enable FDIR filter mode - tunnel.
}

impl convert::From<i32> for RteFdirMode {
    fn from(i: i32) -> RteFdirMode {
        match i {
            0 => RteFdirMode::RteFdirModeNone,
            1 => RteFdirMode::RteFdirModeSignature,
            2 => RteFdirMode::RteFdirModePerfect,
            3 => RteFdirMode::RteFdirModePerfectMacVlan,
            4 => RteFdirMode::RteFdirModePerfectTunnel,
            _ => RteFdirMode::RteFdirModeNone,
        }
    }
}

#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub enum RteFdirPballocType {
    RteFdirPballoc64k = 0,  // 64k.
    RteFdirPballoc128k = 1, // 128k.
    RteFdirPballoc256k = 2, // 256k.
}

impl convert::From<i32> for RteFdirPballocType {
    fn from(i: i32) -> RteFdirPballocType {
        match i {
            0 => RteFdirPballocType::RteFdirPballoc64k,
            1 => RteFdirPballocType::RteFdirPballoc128k,
            2 => RteFdirPballocType::RteFdirPballoc256k,
            _ => RteFdirPballocType::RteFdirPballoc64k,
        }
    }
}

/**
 * Payload type
 */
#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub enum RteEthPayloadType {
    RteEthPayloadUnknown = 0,
    RteEthRawPayload = 1,
    RteEthL2Payload = 2,
    RteEthL3Payload = 3,
    RteEthL4Payload = 4,
    RteEthPayloadMax = 8,
}

/// returned by eth_tx_descriptor_status()
pub enum RteEthTxDescriptorStatus {
    /**< Desc filled for hw, waiting xmit. */
    RteEthTxDescFull = 0,
    /**< Desc done, packet is transmitted. */
    RteEthTxDescDone = 1,
    /**< Desc used by driver or hw. */
    RteEthTxDescUnavail = 2,
}

/// returned by eth_rx_descriptor_status()
pub enum RteEthRxDescriptorStatus {
    /**< Desc available for hw. */
    RteEthRxDescAvail = 0,
    /**< Desc done, filled by hw. */
    RteEthRxDescDone = 1,
    /**< Desc used by driver or hw. */
    RteEthRxDescUnavail = 2,
}

/**
 *  Select report mode of FDIR hash information in RX descriptors.
 */
#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub enum RteFdirStatusMode {
    RteFdirNoReportStatus = 0,     // Never report FDIR hash.
    RteFdirReportStatus = 1,       // Only report FDIR hash for matching pkts.
    RteFdirReportStatusAlways = 2, // Always report FDIR hash.
}

impl convert::From<i32> for RteFdirStatusMode {
    fn from(i: i32) -> RteFdirStatusMode {
        match i {
            0 => RteFdirStatusMode::RteFdirNoReportStatus,
            1 => RteFdirStatusMode::RteFdirReportStatus,
            2 => RteFdirStatusMode::RteFdirReportStatusAlways,
            _ => RteFdirStatusMode::RteFdirNoReportStatus,
        }
    }
}

/**
 * A structure used to select bytes extracted from the protocol layers to
 * flexible payload for filter
 */

const RTE_ETH_FDIR_MAX_FLEXLEN: usize = 16;
const RTE_ETH_FLOW_MAX: usize = 22;

#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub struct RteEthFlexPayloadCfg {
    payloadtype: RteEthPayloadType,
    /**< Payload type */

    /**< Offset in bytes from the beginning of packet's payload
    src_offset[i] indicates the flexbyte i's offset in original
    packet payload. This value should be less than
    flex_payload_limit in struct rte_eth_fdir_info.*/
    src_offset: [u16; RTE_ETH_FDIR_MAX_FLEXLEN],
}

/**
 * A structure used to define FDIR masks for flexible payload
 * for each flow type
 */
#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub struct RteEthFdirFlexMask {
    flow_type: u16,
    mask: [u8; RTE_ETH_FDIR_MAX_FLEXLEN], // Mask for the whole flexible payload
}

/**
 * A structure used to define all flexible payload related setting
 * include flex payload and flex mask
 */
#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub struct RteEthFdirFlexConf {
    nb_payloads: u16,  // The number of following payload cfg
    nb_flexmasks: u16, // The number of following mask */
    flex_set: [RteEthFlexPayloadCfg; RteEthPayloadType::RteEthPayloadMax as usize],
    // Flex payload configuration for each payload type
    flex_mask: [RteEthFdirFlexMask; RTE_ETH_FLOW_MAX],
    // Flex mask configuration for each flow type
}

#[derive(Deserialize, Clone, Copy)]
#[repr(C)]
pub struct RteFdirConf {
    pub mode: RteFdirMode,           // Flow Director mode.
    pub pballoc: RteFdirPballocType, // Space for FDIR filters.
    pub status: RteFdirStatusMode,   // How to report FDIR hash.
    // RX queue of packets matching a "drop" filter in perfect mode.
    pub drop_queue: u8,
    pub mask: RteEthFdirMasks,
    pub flex_conf: RteEthFdirFlexConf,
    // Flex payload configuration.
}

impl RteFdirConf {
    pub fn new() -> RteFdirConf {
        // creates (almost) empty RteFdirConf
        RteFdirConf {
            mode: RteFdirMode::RteFdirModePerfect,
            pballoc: RteFdirPballocType::RteFdirPballoc64k,
            status: RteFdirStatusMode::RteFdirNoReportStatus,
            drop_queue: 63,
            mask: RteEthFdirMasks {
                vlan_tci_mask: 0,
                ipv4_mask: RteEthIpv4Flow {
                    src_ip: 0,
                    dst_ip: 0,
                    tos: 0,
                    ttl: 0,
                    proto: 0,
                },
                ipv6_mask: RteEthIpv6Flow {
                    src_ip: [0u32; 4],
                    dst_ip: [0u32; 4],
                    tc: 0,
                    proto: 0,
                    hop_limits: 0,
                },
                src_port_mask: 0,
                dst_port_mask: 0,
                mac_addr_byte_mask: 0,
                tunnel_id_mask: 0,
                tunnel_type_mask: 0,
            },
            flex_conf: RteEthFdirFlexConf {
                nb_payloads: 0,
                nb_flexmasks: 0,
                flex_set: [RteEthFlexPayloadCfg {
                    payloadtype: RteEthPayloadType::RteEthPayloadUnknown,
                    src_offset: [0u16; RTE_ETH_FDIR_MAX_FLEXLEN],
                }; RteEthPayloadType::RteEthPayloadMax as usize],
                flex_mask: [RteEthFdirFlexMask {
                    flow_type: 0,
                    mask: [0u8; RTE_ETH_FDIR_MAX_FLEXLEN],
                }; RTE_ETH_FLOW_MAX],
            },
        }
    }
}

impl fmt::Display for RteFdirConf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RteFdirConf(mode = {}, pballoc= {}, ipv4_mask= {}. src_port_mask= {:x}, dst_port_mask= {:x})",
            self.mode as i32,
            self.pballoc as i32,
            self.mask.ipv4_mask,
            self.mask.src_port_mask,
            self.mask.dst_port_mask
        )
    }
}

pub fn check_os_error(code: i32) -> io::Result<i32> {
    if code < 0 {
        Err(io::Error::from_raw_os_error(-code))
    } else {
        Ok(code)
    }
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
        mbuf_cnt: u32,
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
        port: u16,
        rxqs: u16,
        txqs: u16,
        rx_cores: *const i32,
        tx_cores: *const i32,
        nrxd: u16,
        ntxd: u16,
        loopback: i32,
        tso: i32,
        csumoffload: i32,
        fdir_conf_ptr: *const RteFdirConf,
    ) -> i32;
    pub fn free_pmd_port(port: u16) -> i32;
    pub fn fdir_get_infos(pmdport_id: u16);
    pub fn eth_rx_burst(port: u16, qid: u16, pkts: *mut *mut MBuf, len: u16) -> u32;
    pub fn eth_rx_queue_count(port_id: u16, queue_id: u16) -> i32;
    // rte_eth_tx_burst is inline C, we cannot directly use it here:
    pub fn eth_tx_burst(port: u16, qid: u16, pkts: *mut *mut MBuf, len: u16) -> u16;
    pub fn eth_tx_descriptor_status(port: u16, qid: u16, offset: u16) -> i32;
    pub fn eth_rx_descriptor_status(port: u16, qid: u16, offset: u16) -> i32;
    pub fn eth_tx_prepare(port: u16, qid: u16, pkts: *mut *mut MBuf, len: u16) -> u16;
    pub fn num_pmd_ports() -> i32;
    pub fn rte_eth_macaddr_get(port: u16, address: *mut MacAddress);
    pub fn init_bess_eth_ring(ifname: *const c_char, core: i32) -> i32;
    pub fn init_ovs_eth_ring(iface: i32, core: i32) -> i32;
    //pub fn find_port_with_pci_address(pciaddr: *const c_char) -> i32;
    //pub fn attach_pmd_device(dev: *const c_char) -> i32;
    /// returns portids of all matching ports for the device
    pub fn attach_device(identifier: *const c_char, port_id_ptr: *mut u16, max_port_ids: u32) -> i32;
    // TODO: Generic PMD info
    pub fn max_rxqs(port: u16) -> i32;
    pub fn max_txqs(port: u16) -> i32;
    pub fn mbuf_alloc() -> *mut MBuf;
    pub fn mbuf_free(buf: *mut MBuf);
    pub fn mbuf_alloc_bulk(array: *mut *mut MBuf, cnt: u32) -> i32;
    pub fn mbuf_free_bulk(array: *mut *mut MBuf, cnt: i32) -> i32;
    pub fn mbuf_avail_count() -> u32;
    pub fn crc_hash_native(to_hash: *const u8, size: u32, iv: u32) -> u32;
    pub fn ipv4_cksum(payload: *const u8) -> u16;
    pub fn ipv4_phdr_chksum(ipv4_hdr: *const IpHeader, ol_flags: u64) -> u16;
    pub fn validate_tx_offload(m: *const MBuf) -> i32;

    //usually called already by rte_eal_init when e.g. --vdev netkni0:
    pub fn rte_kni_init(max_kni_ifaces: u32);
    pub fn kni_alloc(port_id: u16, kni_port_params: *mut KniPortParams) -> *mut RteKni;
    pub fn rte_kni_release(kni: *mut RteKni) -> i32;
    pub fn rte_kni_handle_request(kni: *mut RteKni) -> i32;
    pub fn rte_kni_rx_burst(kni: *mut RteKni, pkts: *mut *mut MBuf, len: u32) -> u32;
    pub fn rte_kni_tx_burst(kni: *mut RteKni, pkts: *mut *mut MBuf, len: u32) -> u32;
    pub fn rte_kni_get_name(kni: *const RteKni) -> *const c_char;
    pub fn rte_kni_update_link(kni: *mut RteKni, linkup: u32) -> i32;

    pub fn rte_log_set_global_level(level: RteLogLevel);
    pub fn rte_log_get_global_level() -> u32;
    pub fn rte_log_set_level(logtype: RteLogtype, level: RteLogLevel) -> i32;
    pub fn rte_log_get_level(logtype: RteLogtype) -> i32;
    pub fn add_tcp_flow(
        port_id: u16,
        rx_q: u16,
        src_ip: u32,
        src_mask: u32,
        dest_ip: u32,
        dest_mask: u32,
        src_port: u16,
        src_port_mask: u16,
        dst_port: u16,
        dst_port_mask: u16,
        error: *const RteFlowError,
    ) -> *mut RteFlow;
    pub fn rte_eth_dev_info_get(port_id: u16, dev_info: *mut rte_eth_dev_info);
    pub fn rte_eth_dev_filter_supported(port_id: u16, filter_type: RteFilterType) -> i32;
    pub fn rte_eth_dev_filter_ctrl(
        port_id: u16,
        filter_type: RteFilterType,
        filter_op: RteFilterOp,
        arg: *mut c_void,
    ) -> i32;

    pub fn rte_eth_xstats_get_names_by_id(
        port_id: u16,
        xstats_names: *const RteEthXstatName,
        size: u32,
        ids: *const u64,
    ) -> i32;
    pub fn rte_eth_xstats_get_by_id(port_id: u16, ids: *const u64, values: *const u64, size: u32) -> i32;
    pub fn rte_eth_xstats_get_id_by_name(port_id: u16, xstat_name: *const c_char, id: *const u64) -> i32;

    pub fn rte_eth_stats_get(port_id: u16, stats: *const RteEthStats) -> i32;
    pub fn rte_eth_stats_reset(port_id: u16) -> i32;

}
