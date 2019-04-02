use super::act::Act;
use super::iterator::*;
use super::packet_batch::PacketBatch;
use super::Batch;
use common::*;
use interface::{PacketTx, Pdu};


pub struct DropBatch<V>
    where
        V: Batch + BatchIterator + Act,
{
    parent: V,
}

impl<V> DropBatch<V>
    where
        V: Batch + BatchIterator + Act,
{
    pub fn new(parent: V) -> DropBatch<V> {
        DropBatch {
            parent: parent,
        }
    }
}

impl<V> Batch for DropBatch<V>
    where
        V: Batch + BatchIterator + Act,
{
    fn queued(&self) -> usize {
        self.parent.queued()
    }
}

impl<V> Act for DropBatch<V>
    where
        V: Batch + BatchIterator + Act,
{
    #[inline]
    fn act(&mut self) -> (u32, i32) {
        let q_len = self.parent.act().1;
        match self.parent.drop_packets_all() {
            Some(dropped) => (dropped as u32, q_len),
            None => { warn!("failed to drop packet batch"); (0,q_len) }
        }
    }

    #[inline]
    fn done(&mut self) {
        self.parent.done();
    }

    #[inline]
    fn send_q(&mut self, port: &mut PacketTx) -> Result<u32> {
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

impl<V> BatchIterator for DropBatch<V>
    where
        V: Batch + BatchIterator + Act,
{
    #[inline]
    fn start(&mut self) -> usize {
        self.parent.start()
    }

    #[inline]
    fn next_payload(&mut self, idx: usize) -> Option<Pdu>{
        self.parent.next_payload(idx)
    }
}
