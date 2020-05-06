use super::act::Act;
use super::iterator::*;
use super::packet_batch::PacketBatch;
use super::Batch;
use common::*;
use interface::PacketTx;
use interface::Pdu;

pub type FilterFn = Box<dyn FnMut(&Pdu) -> bool + Send>;

pub struct FilterBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    parent: V,
    filter: FilterFn,
    capacity: usize,
    remove: Vec<usize>,
}

impl<V> FilterBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    #[inline]
    pub fn new(parent: V, filter: FilterFn) -> FilterBatch<V> {
        let capacity = parent.capacity() as usize;
        FilterBatch {
            parent,
            filter,
            capacity,
            remove: Vec::with_capacity(capacity),
        }
    }
}

batch_no_new! {FilterBatch}

impl<V> Act for FilterBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    #[inline]
    fn act(&mut self) -> (u32, i32) {
        let mut count = 0;
        let pre = self.parent.act();
        // Filter during the act
        let iter = PayloadEnumerator::new(&mut self.parent);
        while let Some(ParsedDescriptor { index: idx, mut pdu }) = iter.next(&mut self.parent) {
            if !(self.filter)(&mut pdu) {
                self.remove.push(idx)
            }
            count += 1;
        }
        if !self.remove.is_empty() {
            self.parent
                .drop_packets(&self.remove[..])
                .expect("Filtering was performed incorrectly");
        }
        self.remove.clear();
        (count, pre.1)
    }

    #[inline]
    fn done(&mut self) {
        self.parent.done();
    }

    #[inline]
    fn send_q(&mut self, port: &mut dyn PacketTx) -> errors::Result<u32> {
        self.parent.send_q(port)
    }

    #[inline]
    fn capacity(&self) -> i32 {
        self.capacity as i32
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

impl<V> BatchIterator for FilterBatch<V>
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
