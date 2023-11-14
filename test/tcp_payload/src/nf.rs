//! This NF reconstructs TCP flows. The entire payload is printed when a FIN packet is received.

use e2d2::operators::*;
use e2d2::scheduler::*;
use e2d2::state::*;
use e2d2::utils::FiveTupleV4;
use fnv::FnvHasher;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use uuid::Uuid;

type FnvHash = BuildHasherDefault<FnvHasher>;
const BUFFER_SIZE: usize = 2048;
const READ_SIZE: usize = 256;

fn read_payload(
    rb: &mut ReorderedBuffer,
    to_read: usize,
    flow: FiveTupleV4,
    payload_cache: &mut HashMap<FiveTupleV4, Vec<u8>>,
) {
    let mut read_buf = [0; READ_SIZE];
    let mut so_far = 0;
    while to_read > so_far {
        let payload = payload_cache.entry(flow).or_insert(Vec::new());
        let n = rb.read_data(&mut read_buf);
        so_far += n;
        payload.extend(&read_buf[..n]);
    }
}

pub fn reconstruction<T: 'static + Batch, S: Scheduler + Sized>(parent: T, sched: &mut S) -> CompositionBatch {
    let mut rb_map = HashMap::<FiveTupleV4, ReorderedBuffer, FnvHash>::with_hasher(Default::default());
    let mut payload_cache = HashMap::<FiveTupleV4, Vec<u8>>::with_hasher(Default::default());
    let uuid = Uuid::new_v4();
    let mut groups = parent
        .transform(Box::new(move |p| {
            p.headers_mut().mac_mut(0).swap_addresses();
        }))
        .group_by(
            2,
            Box::new(move |p| if p.headers().ip(1).protocol() == 6 { 0 } else { 1 }),
            sched,
            "GroupByProtocol".to_string(),
            uuid,
        );
    let pipe = groups
        .get_group(0)
        .unwrap()
        .transform(Box::new(move |p| {
            let flow = p.headers().ip(1).flow().unwrap();
            let mut seq = p.headers().tcp(2).seq_num();
            match rb_map.entry(flow) {
                Entry::Occupied(mut e) => {
                    {
                        let b = e.get_mut();
                        let result = b.add_data(seq, p.get_payload(2));
                        match result {
                            InsertionResult::Inserted { available, .. } => {
                                read_payload(b, available, flow, &mut payload_cache);
                            }
                            InsertionResult::OutOfMemory { written, .. } => {
                                if written == 0 {
                                    println!("Resetting since receiving data that is too far ahead");
                                    b.reset();
                                    b.seq(seq, p.get_payload(2));
                                }
                            }
                        }
                    }
                    if p.headers().tcp(2).rst_flag() {
                        e.remove_entry();
                    } else if p.headers().tcp(2).fin_flag() {
                        match payload_cache.entry(flow) {
                            Entry::Occupied(e) => {
                                let (_, payload) = e.remove_entry();
                                println!("{}", String::from_utf8_lossy(&payload));
                            }
                            Entry::Vacant(_) => {
                                println!("dumped an empty payload for Flow={:?}", flow);
                            }
                        }
                        e.remove_entry();
                    }
                }
                Entry::Vacant(e) => {
                    match ReorderedBuffer::new(BUFFER_SIZE) {
                        Ok(mut b) => {
                            if p.headers().tcp(2).syn_flag() {
                                seq += 1; // Receiver should expect data beginning at seq+1.
                            } else {
                                println!("packet received for untracked flow did not have SYN flag, skipping.");
                            }

                            let result = b.seq(seq, p.get_payload(2));
                            match result {
                                InsertionResult::Inserted { available, .. } => {
                                    read_payload(&mut b, available, flow, &mut payload_cache);
                                }
                                InsertionResult::OutOfMemory { .. } => {
                                    println!("Too big a packet?");
                                }
                            }
                            e.insert(b);
                        }
                        Err(_) => (),
                    }
                }
            }
        }))
        .compose();
    merge(vec![pipe, groups.get_group(1).unwrap().compose()]).compose()
}
