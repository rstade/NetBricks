use super::act::Act;
use super::iterator::*;
use super::packet_batch::PacketBatch;
use super::Batch;
use common::*;
use interface::Pdu;
use interface::PacketTx;

pub type TransformFn = Box<FnMut(&mut Pdu) + Send>;

pub struct TransformBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    parent: V,
    transformer: TransformFn,
    applied: bool,
}

impl<V> TransformBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    pub fn new(parent: V, transformer: TransformFn) -> TransformBatch<V> {
        TransformBatch {
            parent,
            transformer,
            applied: false,
        }
    }
}

impl<V> Batch for TransformBatch<V>
where
    V: Batch + BatchIterator + Act,
{
    fn queued(&self) -> usize {
        self.parent.queued()
    }
}

impl<V> BatchIterator for TransformBatch<V>
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

impl<V> Act for TransformBatch<V>
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
                while let Some(ParsedDescriptor { mut pdu, .. }) = iter.next(&mut self.parent) {
                    (self.transformer)(&mut pdu);
                    count += 1
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

    //    #[inline]
    //    fn get_task_dependencies(&self) -> Vec<usize> {
    //        self.parent.get_task_dependencies()
    //    }
}
