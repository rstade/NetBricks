use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Arc;

use super::PmdPort;
use native::zcsi::rte_ethdev_api::rte_flow;
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
    //    rte_flows: HashMap<u16, *mut rte_flow>,
}

impl FlowDirector {
    pub fn new(pmd_port: Arc<PmdPort>) -> FlowDirector {
        FlowDirector {
            pmd_port,
            flows: HashMap::new(),
            //            rte_flows: HashMap::new(),
        }
    }

    pub fn get_flow(&self, rxq: u16) -> &L4Flow {
        self.flows.get(&rxq).unwrap()
    }

    pub fn add_tcp_flow_rule(&mut self, rxq: u16, dst_ip: u32, dst_mask: u32, dst_port: u16, dst_port_mask: u16) {
        let error: *const RteFlowError = &mut RteFlowError::new();
        self.flows.insert(
            rxq,
            L4Flow {
                ip: dst_ip,
                port: dst_port,
            },
        );
        unsafe {
            let _flow: *mut rte_flow = add_tcp_flow(
                self.pmd_port.port_id() as u16,
                rxq,
                0u32,
                0u32,
                dst_ip,
                dst_mask,
                0u16,
                0u16,
                dst_port,
                dst_port_mask,
                error,
            );
            //            self.rte_flows.insert(rxq, flow);
            debug!(
                "Flowdirector added tcp flow: Pmdport id= {}, rx-queue={}, destination=({},{:#X}), mask=({:#X},{:#X}))",
                self.pmd_port.port_id(),
                rxq,
                Ipv4Addr::from(dst_ip),
                dst_port,
                dst_mask,
                dst_port_mask
            )
        }
    }
}
