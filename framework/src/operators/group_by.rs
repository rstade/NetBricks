use super::act::Act;
use super::iterator::*;
use super::Batch;
use super::ReceiveBatch;
use interface::Pdu;
use queues::*;
use scheduler::{Executable, Runnable, Scheduler};
use std::collections::HashMap;
use std::marker::PhantomData;
use uuid::Uuid;

pub type GroupFnPdu = Box<dyn FnMut(&mut Pdu) -> usize>;

pub struct GroupBy<V>
where
    V: Batch + BatchIterator + Act + 'static,
{
    _phantom_v: PhantomData<V>,
    groups: usize,
    consumers: HashMap<usize, ReceiveBatch<MpscConsumer>>,
}

struct GroupByProducer<V>
where
    V: Batch + BatchIterator + Act + 'static,
{
    parent: V,
    producers: Vec<MpscProducer>,
    group_fn: GroupFnPdu,
}

impl<V> Executable for GroupByProducer<V>
where
    V: Batch + BatchIterator + Act + 'static,
{
    #[inline]
    fn execute(&mut self) -> (u32, i32) {
        let mut count = 0;
        let pre = self.parent.act(); // Let the parent get some packets.
        {
            let iter = PayloadEnumerator::new(&mut self.parent);
            while let Some(ParsedDescriptor { mut pdu, .. }) = iter.next(&mut self.parent) {
                //let group = (self.group_fn)(&mut packet);
                let group = (self.group_fn)(&mut pdu);
                if !self.producers[group].enqueue_one(pdu) {
                    warn!("queue overflow in GroupByProducer for group {}", group);
                }
                count += 1;
            }
        }
        self.parent.get_packet_batch().clear_packets();
        self.parent.done();
        (count, pre.1)
    }
}

#[cfg_attr(feature = "dev", allow(len_without_is_empty))]
impl<V> GroupBy<V>
where
    V: Batch + BatchIterator + Act + 'static,
{
    pub fn new<S: Scheduler + Sized>(
        parent: V,
        groups: usize,
        group_fn: GroupFnPdu,
        sched: &mut S,
        name: String,
        uuid: Uuid, // task id
    ) -> GroupBy<V> {
        let mut producers = Vec::with_capacity(groups);
        let mut consumers = HashMap::with_capacity(groups);
        for i in 0..groups {
            let (prod, consumer) = new_mpsc_queue_pair();
            producers.push(prod);
            consumers.insert(i, consumer);
        }
        let _task = sched.add_runnable(
            Runnable::from_task(
                uuid,
                name,
                GroupByProducer {
                    parent,
                    group_fn,
                    producers,
                },
            )
            .move_unready(),
        );
        GroupBy {
            _phantom_v: PhantomData,
            groups,
            consumers,
        }
    }

    pub fn len(&self) -> usize {
        self.groups
    }

    pub fn get_group(&mut self, group: usize) -> Option<ReceiveBatch<MpscConsumer>> {
        self.consumers.remove(&group)
    }
}
