use super::{Batch, BatchIterator, PacketBatch, Act};
use super::super::interface::{Pdu, PacketTx };
use super::super::common::errors;
use scheduler::Executable;

/// `CompositionBatch` allows multiple NFs to be combined.
///
pub struct CompositionBatch {
    parent: Box<Batch>,
}

impl CompositionBatch {
    pub fn new<V: 'static + Batch>(
        parent: V,
    ) -> CompositionBatch {
        CompositionBatch {
            parent: box parent,
        }
    }
}

impl Batch for CompositionBatch {
    fn queued(&self) -> usize { self.parent.queued() }
}

impl BatchIterator for CompositionBatch {

    #[inline]
    fn start(&mut self) -> usize {
        self.parent.start()
    }

    #[inline]
    fn next_payload(&mut self, idx: usize) -> Option<Pdu> {
        self.parent.next_payload(idx)
    }
}

/// Internal interface for packets.
impl Act for CompositionBatch {
    #[inline]
    fn act(&mut self) -> (u32, i32) {
        self.parent.act()
    }

    #[inline]
    fn done(&mut self) {
        self.parent.done();
    }

    #[inline]
    fn send_q(&mut self, port: &mut PacketTx) -> errors::Result<u32> {
        self.parent.send_q(port)
    }

    #[inline]
    fn capacity(&self) -> i32 {
        self.parent.capacity()
    }

    #[inline]
    fn drop_packets(&mut self, idxes: &[usize]) -> Option<usize> {
        self.parent.drop_packets(idxes)
    }

    #[inline]
    fn drop_packets_all(&mut self) -> Option<usize> {
        self.parent.drop_packets_all()
    }

    #[inline]
    fn clear_packets(&mut self) {
        self.parent.clear_packets()
    }

    #[inline]
    fn get_packet_batch(&mut self) -> &mut PacketBatch {
        self.parent.get_packet_batch()
    }
}

impl Executable for CompositionBatch {
    #[inline]
    fn execute(&mut self) -> (u32, i32) {
        let count= self.act();
        self.done();
        count
    }

}