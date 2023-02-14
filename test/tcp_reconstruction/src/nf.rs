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
const PRINT_SIZE: usize = 256;

pub fn reconstruction<T: 'static + Batch, S: Scheduler + Sized>(parent: T, sched: &mut S) -> CompositionBatch {
    let mut cache = HashMap::<FiveTupleV4, ReorderedBuffer, FnvHash>::with_hasher(Default::default());
    let mut read_buf: Vec<u8> = (0..PRINT_SIZE).map(|_| 0).collect();
    let uuid = Uuid::new_v4();
    let mut groups = parent
        .transform(box move |p| {
            p.headers_mut().mac_mut(0).swap_addresses();
        })
        .group_by(
            2,
            box move |p| if p.headers().ip(1).protocol() == 6 { 0 } else { 1 },
            sched,
            "GroupByProtocol".to_string(),
            uuid,
        );
    let pipe = groups
        .get_group(0)
        .unwrap()
        .transform(box move |p| {
            if !p.headers().tcp(2).psh_flag() {
                let flow = p.headers().ip(1).flow().unwrap();
                let seq = p.headers().tcp(2).seq_num();
                match cache.entry(flow) {
                    Entry::Occupied(mut e) => {
                        let reset = p.headers().tcp(2).rst_flag();
                        {
                            let entry = e.get_mut();
                            let result = entry.add_data(seq, p.get_payload(2));
                            match result {
                                InsertionResult::Inserted { available, .. } => {
                                    if available > PRINT_SIZE {
                                        let mut read = 0;
                                        while available - read > PRINT_SIZE {
                                            let avail = entry.read_data(&mut read_buf[..]);
                                            read += avail;
                                        }
                                    }
                                }
                                InsertionResult::OutOfMemory { written, .. } => {
                                    if written == 0 {
                                        // println!("Resetting since receiving data that is too far ahead");
                                        entry.reset();
                                        entry.seq(seq, p.get_payload(2));
                                    }
                                }
                            }
                        }
                        if reset {
                            // Reset handling.
                            e.remove_entry();
                        }
                    }
                    Entry::Vacant(e) => match ReorderedBuffer::new(BUFFER_SIZE) {
                        Ok(mut b) => {
                            if !p.headers().tcp(2).syn_flag() {}
                            let result = b.seq(seq, p.get_payload(2));
                            match result {
                                InsertionResult::Inserted { available, .. } => {
                                    if available > PRINT_SIZE {
                                        let mut read = 0;
                                        while available - read > PRINT_SIZE {
                                            let avail = b.read_data(&mut read_buf[..]);
                                            read += avail;
                                        }
                                    }
                                }
                                InsertionResult::OutOfMemory { .. } => {
                                    println!("Too big a packet?");
                                }
                            }
                            e.insert(b);
                        }
                        Err(_) => (),
                    },
                }
            }
        })
        .compose();
    merge(vec![pipe, groups.get_group(1).unwrap().compose()]).compose()
}
