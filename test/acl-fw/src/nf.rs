use e2d2::operators::*;
use e2d2::utils::{FiveTupleV4, Ipv4Prefix};
use fnv::FnvHasher;
use std::collections::HashSet;
use std::hash::BuildHasherDefault;

type FnvHash = BuildHasherDefault<FnvHasher>;

#[derive(Clone)]
pub struct Acl {
    pub src_ip: Option<Ipv4Prefix>,
    pub dst_ip: Option<Ipv4Prefix>,
    pub src_port: Option<u16>,
    pub dst_port: Option<u16>,
    pub established: Option<bool>,
    // Related not done
    pub drop: bool,
}

impl Acl {
    pub fn matches(&self, flow: &FiveTupleV4, connections: &HashSet<FiveTupleV4, FnvHash>) -> bool {
        if (self.src_ip.is_none() || self.src_ip.unwrap().in_range(flow.src_ip))
            && (self.dst_ip.is_none() || self.dst_ip.unwrap().in_range(flow.dst_ip))
            && (self.src_port.is_none() || flow.src_port == self.src_port.unwrap())
            && (self.dst_port.is_none() || flow.dst_port == self.dst_port.unwrap())
        {
            if let Some(established) = self.established {
                let rev_flow = flow.reverse_flow();
                (connections.contains(flow) || connections.contains(&rev_flow)) == established
            } else {
                true
            }
        } else {
            false
        }
    }
}

pub fn acl_match<T: 'static + Batch>(parent: T, acls: Vec<Acl>) -> CompositionBatch {
    let mut flow_cache = HashSet::<FiveTupleV4, FnvHash>::with_hasher(Default::default());
    parent
        .transform(Box::new(move |p| {
            p.headers_mut().mac_mut(0).swap_addresses();
        }))
        .filter(Box::new(move |p| {
            let flow = p.headers().ip(1).flow().unwrap();
            for acl in &acls {
                if acl.matches(&flow, &flow_cache) {
                    if !acl.drop {
                        flow_cache.insert(flow);
                    }
                    return !acl.drop;
                }
            }
            return false;
        }))
        .compose()
}
