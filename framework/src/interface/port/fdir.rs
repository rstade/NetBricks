use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::Arc;

use super::PmdPort;
use config::DriverType;
use native::zcsi::*;

#[derive(Clone, Copy)]
pub struct L4Flow {
    pub ip: u32,
    pub port: u16,
}


#[derive(Deserialize, Clone, Copy, PartialEq)]
pub enum FlowSteeringMode {
    // Port is default
    Port,
    Ip,
}


pub struct FlowDirector {
    pmd_port: Arc<PmdPort>,
    flows: HashMap<u16, L4Flow>,
}

impl FlowDirector {
    pub fn new(pmd_port: Arc<PmdPort>) -> FlowDirector {
        FlowDirector {
            pmd_port,
            flows: HashMap::new(),
        }
    }

    pub fn get_flow(&self, rxq: u16) -> &L4Flow {
        self.flows.get(&rxq).unwrap()
    }

    pub fn add_fdir_filter(&mut self, rxq: u16, dst_ip: u32, dst_port: u16) -> std::io::Result<i32> {
        self.flows.insert(
            rxq,
            L4Flow {
                ip: dst_ip,
                port: dst_port,
            },
        );
        if self.pmd_port.driver() == DriverType::I40e {
            self.add_fdir_filter_i40e(rxq, dst_ip, dst_port)
        } else {
            self.add_fdir_filter_ixgbe(rxq, dst_ip, dst_port)
        }
    }

    fn add_fdir_filter_ixgbe(&self, rxq: u16, dst_ip: u32, dst_port: u16) -> std::io::Result<i32> {
        // assumes that flows in Fdir are fully masked, except for the destination ip and port

        let action = RteEthFdirAction {
            rx_queue: rxq,
            behavior: RteEthFdirBehavior::RteEthFdirAccept,
            report_status: RteEthFdirStatus::RteEthFdirNoReportStatus,
            flex_off: 0,
        };

        let ip = RteEthIpv4Flow {
            src_ip: 0,
            dst_ip: u32::to_be(dst_ip),
            tos: 0,
            ttl: 0,
            proto: 6,
        };

        let flow_ext = RteEthFdirFlowExt {
            vlan_tci: 0u16,
            flexbytes: [0u8; 16],
            is_vf: 0u8,   // 1 for VF, 0 for port dev
            dst_id: 0u16, // VF ID, available when is_vf is 1
        };

        let flow = RteEthTcpv4Flow {
            ip,                             // < IPv4 fields to match.
            src_port: 0u16,                 // < TCP source port in big endian.
            dst_port: u16::to_be(dst_port), // < TCP destination port in big endian.
            _padding: [0u8; 28],
        };

        let input = RteEthFdirInputTcpv4 {
            flow_type: 4, // i.e. RTE_ETH_FLOW_NONFRAG_IPV4_TCP
            flow,
            flow_ext,
        };

        let mut fdir_filter = RteEthFdirFilter {
            soft_id: 0,
            input,
            action,
        };

        let fdir_filter_ptr: *mut RteEthFdirFilter = &mut fdir_filter;

        unsafe {
            check_os_error(rte_eth_dev_filter_ctrl(
                self.pmd_port.port_id() as u16,
                RteFilterType::RteEthFilterFdir,
                RteFilterOp::RteEthFilterAdd,
                fdir_filter_ptr as *mut c_void,
            ))
        }
    }

    fn add_fdir_filter_i40e(&self, rxq: u16, dst_ip: u32, _dst_port: u16) -> std::io::Result<i32> {
        unsafe {
            check_os_error(rte_eth_dev_filter_supported(
                self.pmd_port.port_id() as u16,
                RteFilterType::RteEthFilterFdir,
            ))?;
        }

        let mut filter_info = RteEthFdirFilterInfo {
            info_type: RteEthFdirFilterInfoType::RteEthFdirFilterInputSetSelect,
            input_set_conf: RteEthInputSetConf {
                flow_type: RTE_ETH_FLOW_NONFRAG_IPV4_TCP, // RTE_ETH_FLOW_FRAG_IPV4
                inset_size: 1u16,
                field: [RteEthInputSetField::Unknown; RTE_ETH_INSET_SIZE_MAX],
                op: RteFilterInputSetOp::Select,
            },
        };
        filter_info.input_set_conf.field[0] = RteEthInputSetField::L3DstIp4;
        let fdir_filter_info: *mut RteEthFdirFilterInfo = &mut filter_info;

        unsafe {
            check_os_error(rte_eth_dev_filter_ctrl(
                self.pmd_port.port_id() as u16,
                RteFilterType::RteEthFilterFdir,
                RteFilterOp::RteEthFilterSet,
                fdir_filter_info as *mut c_void,
            ))?;
        }

        let action = RteEthFdirAction {
            rx_queue: rxq,
            behavior: RteEthFdirBehavior::RteEthFdirAccept,
            report_status: RteEthFdirStatus::RteEthFdirNoReportStatus,
            flex_off: 0,
        };

        let ip = RteEthIpv4Flow {
            src_ip: 0,
            dst_ip: u32::to_be(dst_ip),
            tos: 0,
            ttl: 0,
            proto: 6,
        };

        #[repr(C)]
        pub struct PaddedIpv4Flow {
            ip: RteEthIpv4Flow,
            _padding: [u8; 32],
        }

        let flow = PaddedIpv4Flow {
            ip,
            _padding: [0u8; 32],
        };

        let flow_ext = RteEthFdirFlowExt {
            vlan_tci: 0u16,
            flexbytes: [0u8; 16],
            is_vf: 0u8,   // 1 for VF, 0 for port dev
            dst_id: 0u16, // VF ID, available when is_vf is 1
        };

        #[repr(C)]
        pub struct RteEthFdirInputIpv4 {
            pub flow_type: u16,
            // e.g. RTE_ETH_FLOW_NONFRAG_IPV4_TCP
            pub flow: PaddedIpv4Flow,
            // < Flow fields to match, dependent on flow_type */
            pub flow_ext: RteEthFdirFlowExt,
            // < Additional fields to match */
        }

        let input = RteEthFdirInputIpv4 {
            flow_type: RTE_ETH_FLOW_NONFRAG_IPV4_TCP,
            flow,
            flow_ext,
        };

        #[repr(C)]
        pub struct RteEthFdirFilter {
            pub soft_id: u32,
            /**< ID, an unique value is required when deal with FDIR entry */
            pub input: RteEthFdirInputIpv4,
            // < Input set
            pub action: RteEthFdirAction, // < Action taken when match
        }

        let mut fdir_filter = RteEthFdirFilter {
            soft_id: 0,
            input,
            action,
        };

        let fdir_filter_ptr: *mut RteEthFdirFilter = &mut fdir_filter;

        unsafe {
            check_os_error(rte_eth_dev_filter_ctrl(
                self.pmd_port.port_id() as u16,
                RteFilterType::RteEthFilterFdir,
                RteFilterOp::RteEthFilterAdd,
                fdir_filter_ptr as *mut c_void,
            ))
            .map_err(|e| e.into())
        }
    }
}
