use super::act::Act;
use super::iterator::BatchIterator;
use super::packet_batch::PacketBatch;
use super::Batch;
use super::SchedulingPolicy;

use common::*;
use interface::{PacketTx, Pdu};
use scheduler::Executable;
use std::cmp;

pub struct MergeBatchAuto {
    //queues
    parents: Vec<Box<Batch>>,
    //queue sizes
    state: Vec<usize>,
    //actually selected queue
    which: usize,
    //longest queue
    queue_max: usize,
    //size of longest queue
    queue_size: usize,
    //scheduler function pointer
    select_queue: fn(&mut MergeBatchAuto) -> usize,
}

impl MergeBatchAuto {
    pub fn new(parents: Vec<Box<Batch>>, policy: SchedulingPolicy) -> MergeBatchAuto {
        let select_queue;
        match policy {
            SchedulingPolicy::LongestQueue => {
                select_queue = MergeBatchAuto::longest_queue as fn(&mut MergeBatchAuto) -> usize
            }
            SchedulingPolicy::RoundRobin => {
                select_queue = MergeBatchAuto::round_robin as fn(&mut MergeBatchAuto) -> usize
            }
        }
        let len = parents.len();
        MergeBatchAuto {
            parents,
            state: vec![1; len],
            which: 0,
            queue_size: 0,
            queue_max: 0,
            select_queue,
        }
    }

    #[inline]
    fn update_state(&mut self) {
        let state = &mut self.state;
        let mut max_queue: (usize, usize) = (0, 0);
        self.parents.iter().enumerate().for_each(|(i, batch)| {
            let q = batch.queued();
            state[i] = q;
            if q > max_queue.0 {
                max_queue = (q, i);
            }
        });
        self.queue_max = max_queue.1;
        self.queue_size = max_queue.0;
    }

    // selects next ready parent and returns queue length if a ready parent found
    #[inline]
    fn round_robin(&mut self) -> usize {
        let mut queue = 0;
        for _i in 0..self.state.len() {
            self.which = (self.which + 1) % self.state.len();
            queue = self.state[self.which];
            if queue > 0 {
                break;
            }
        }
        queue
    }

    #[inline]
    fn longest_queue(&mut self) -> usize {
        self.which = self.queue_max;
        self.queue_size
    }
}

impl Batch for MergeBatchAuto {
    #[inline]
    fn queued(&self) -> usize {
        self.queue_size
    }
}

impl BatchIterator for MergeBatchAuto {
    #[inline]
    fn start(&mut self) -> usize {
        self.parents[self.which].start()
    }

    #[inline]
    fn next_payload(&mut self, idx: usize) -> Option<Pdu> {
        self.parents[self.which].next_payload(idx)
    }
}

/// Internal interface for packets.
impl Act for MergeBatchAuto {
    #[inline]
    fn act(&mut self) -> (u32, i32) {
        self.update_state();
        if (self.select_queue)(self) > 0 {
            self.parents[self.which].act()
        } else {
            (0, 0)
        }
    }

    #[inline]
    fn done(&mut self) {
        self.parents[self.which].done();
    }

    #[inline]
    fn send_q(&mut self, port: &mut PacketTx) -> errors::Result<u32> {
        self.parents[self.which].send_q(port)
    }

    #[inline]
    fn capacity(&self) -> i32 {
        self.parents.iter().fold(0, |acc, x| cmp::max(acc, x.capacity()))
    }

    #[inline]
    fn drop_packets(&mut self, idxes: &[usize]) -> Option<usize> {
        self.parents[self.which].drop_packets(idxes)
    }

    #[inline]
    fn drop_packets_all(&mut self) -> Option<usize> {
        self.parents[self.which].drop_packets_all()
    }

    #[inline]
    fn clear_packets(&mut self) {
        self.parents[self.which].clear_packets()
    }

    #[inline]
    fn get_packet_batch(&mut self) -> &mut PacketBatch {
        self.parents[self.which].get_packet_batch()
    }
}

impl Executable for MergeBatchAuto {
    #[inline]
    fn execute(&mut self) -> (u32, i32) {
        let count = self.act();
        self.done();
        count
    }
}
