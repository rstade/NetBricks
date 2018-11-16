use super::act::Act;
use super::iterator::{BatchIterator, PacketDescriptor};
use super::packet_batch::PacketBatch;
use super::Batch;
use common::*;
use interface::PacketTx;
use scheduler::Executable;
use std::cmp;

pub struct MergeBatchAuto<T: Batch> {
    parents: Vec<T>,
    state: Vec<usize>,
    which: usize,
}

impl<T: Batch> MergeBatchAuto<T> {
    pub fn new(parents: Vec<T>) -> MergeBatchAuto<T> {
        let len=parents.len();
        MergeBatchAuto {
            parents,
            state: vec![1; len],
            which: 0,
        }
    }

    #[inline]
    fn update_state(&mut self) {
        let state=&mut self.state;
        self.parents.iter().enumerate().for_each( |(i,batch)| { state[i]=batch.queued() })
    }

    // selects next ready parent and returns queue length if a ready parent found
    #[inline]
    fn find_next(&mut self) -> usize {
        let mut queue = 0;
        for _i in 0..self.state.len() {
            self.which=(self.which+1) % self.state.len();
            queue=self.state[self.which];
            if queue>0 { break }
        }
        queue
    }
}

impl<T: Batch> Batch for MergeBatchAuto<T> {
    #[inline]
    fn queued(&self) -> usize {
        let mut result = 0;
        for b in &self.state {
            if *b>0 {
                result = *b;
                break;
            }
        }
        result
    }
}

impl<T: Batch> BatchIterator for MergeBatchAuto<T> {
    type Header = T::Header;
    type Metadata = T::Metadata;

    #[inline]
    fn start(&mut self) -> usize {
        self.parents[self.which].start()
    }

    #[inline]
    unsafe fn next_payload(&mut self, idx: usize) -> Option<PacketDescriptor<T::Header, T::Metadata>> {
        self.parents[self.which].next_payload(idx)
    }
}

/// Internal interface for packets.
impl<T: Batch> Act for MergeBatchAuto<T> {
    #[inline]
    fn act(&mut self)-> (u32, i32) {
        self.update_state();
        if self.find_next() > 0 {
            self.parents[self.which].act()
        } else { (0, 0) }
    }

    #[inline]
    fn done(&mut self) {
        self.parents[self.which].done();
    }

    #[inline]
    fn send_q(&mut self, port: &PacketTx) -> errors::Result<u32> {
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
    fn clear_packets(&mut self) {
        self.parents[self.which].clear_packets()
    }

    #[inline]
    fn get_packet_batch(&mut self) -> &mut PacketBatch {
        self.parents[self.which].get_packet_batch()
    }

    //    #[inline]
    ////    fn get_task_dependencies(&self) -> Vec<usize> {
    //        let mut deps = Vec::with_capacity(self.parents.len()); // Might actually need to be larger, will get resized
    //        for parent in &self.parents {
    //            deps.extend(parent.get_task_dependencies().iter())
    //        }
    //        // We need to eliminate duplicate tasks. Fortunately this is not called on the critical path so it is fine to do
    //        // it this way.
    //        deps.sort();
    //        deps.dedup();
    //        deps
    //    }
}

impl<T: Batch> Executable for MergeBatchAuto<T> {
    #[inline]
    fn execute(&mut self) -> (u32, i32) {
        let count = self.act();
        self.done();
        count
    }

    //    #[inline]
    //    fn dependencies(&mut self) -> Vec<usize> {
    //        self.get_task_dependencies()
    //    }
}
