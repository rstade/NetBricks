use super::act::Act;
use super::iterator::*;
use super::packet_batch::PacketBatch;
use super::Batch;
use common::*;
use interface::PacketTx;
use interface::Pdu;

pub type MapFn = Box<dyn FnMut(&Pdu) + Send>;

pub struct MapBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    parent: V,
    transformer: MapFn,
    applied: bool,
}

impl<V> MapBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    pub fn new(parent: V, transformer: MapFn) -> MapBatch<V> {
        MapBatch {
            parent: parent,
            transformer: transformer,
            applied: false,
        }
    }
}

impl<V> Batch for MapBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    #[inline]
    fn queued(&self) -> usize {
        self.parent.queued()
    }
}

impl<V> Act for MapBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    #[inline]
    fn act(&mut self) -> (u32, i32) {
        let mut count = 0;
        let mut q_len = 0;
        if !self.applied {
            q_len = self.parent.act().1;
            {
                let iter = PayloadEnumerator::new(&mut self.parent);
                while let Some(ParsedDescriptor { pdu, .. }) = iter.next(&mut self.parent) {
                    (self.transformer)(&pdu);
                    count += 1;
                }
            }
            self.applied = true;
        }
        (count, q_len)
    }

    #[inline]
    fn done(&mut self) {
        self.applied = false;
        self.parent.done();
    }

    #[inline]
    fn send_q(&mut self, port: &mut dyn PacketTx) -> Result<u32> {
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

impl<V> BatchIterator for MapBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    #[inline]
    fn start(&mut self) -> usize {
        self.parent.start()
    }

    #[inline]
    fn next_payload(&mut self, idx: usize) -> Option<Pdu> {
        self.parent.next_payload(idx)
    }
}
