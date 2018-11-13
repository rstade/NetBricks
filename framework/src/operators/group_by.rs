use super::act::Act;
use super::iterator::*;
use super::Batch;
use super::ReceiveBatch;
use super::RestoreHeader;
use headers::EndOffset;
use interface::Packet;
use queues::*;
use scheduler::{Executable, Runnable, Scheduler};
use std::collections::HashMap;
use std::marker::PhantomData;
use uuid::Uuid;

pub type GroupFn<T, M> = Box<FnMut(&mut Packet<T, M>) -> usize + Send>;

pub struct GroupBy<T, V>
where
    T: EndOffset + 'static,
    V: Batch + BatchIterator<Header = T> + Act + 'static,
{
    _phantom_v: PhantomData<V>,
    groups: usize,
    _phantom_t: PhantomData<T>,
    consumers: HashMap<usize, ReceiveBatch<MpscConsumer>>,
}

struct GroupByProducer<T, V>
where
    T: EndOffset + 'static,
    V: Batch + BatchIterator<Header = T> + Act + 'static,
{
    parent: V,
    producers: Vec<MpscProducer>,
    group_fn: GroupFn<T, V::Metadata>,
}

impl<T, V> Executable for GroupByProducer<T, V>
where
    T: EndOffset + 'static,
    V: Batch + BatchIterator<Header = T> + Act + 'static,
{
    #[inline]
    fn execute(&mut self) -> (u32, i32) {
        let mut count = 0;
        let pre= self.parent.act(); // Let the parent get some packets.
        {
            let iter = PayloadEnumerator::<T, V::Metadata>::new(&mut self.parent);
            while let Some(ParsedDescriptor { mut packet, .. }) = iter.next(&mut self.parent) {
                let group = (self.group_fn)(&mut packet);
                packet.save_header_and_offset();
                self.producers[group].enqueue_one(packet);
                count += 1;
            }
        }
        self.parent.get_packet_batch().clear_packets();
        self.parent.done();
        (count, pre.1)
    }

    //    #[inline]
    //    fn dependencies(&mut self) -> Vec<usize> {
    //        self.parent.get_task_dependencies()
    //    }
}

#[cfg_attr(feature = "dev", allow(len_without_is_empty))]
impl<T, V> GroupBy<T, V>
where
    T: EndOffset + 'static,
    V: Batch + BatchIterator<Header = T> + Act + 'static,
{
    pub fn new<S: Scheduler + Sized>(
        parent: V,
        groups: usize,
        group_fn: GroupFn<T, V::Metadata>,
        sched: &mut S,
        uuid: Uuid, // task id
    ) -> GroupBy<T, V> {
        let mut producers = Vec::with_capacity(groups);
        let mut consumers = HashMap::with_capacity(groups);
        for i in 0..groups {
            let (prod, consumer) = new_mpsc_queue_pair();
            producers.push(prod);
            consumers.insert(i, consumer);
        }
        let name = String::from("GroupByProducer");
        let _task = sched.add_runnable(
            Runnable::from_task(
                uuid,
                name,
                GroupByProducer {
                    parent,
                    group_fn,
                    producers,
                },
            ).move_unready(),
        );
        GroupBy {
            _phantom_v: PhantomData,
            groups,
            _phantom_t: PhantomData,
            consumers,
        }
    }

    pub fn len(&self) -> usize {
        self.groups
    }

    pub fn get_group(&mut self, group: usize) -> Option<RestoreHeader<T, V::Metadata, ReceiveBatch<MpscConsumer>>> {
        match self.consumers.remove(&group) {
            Some(p) => {
                {
                    // p.get_packet_batch().add_parent_task(self.task)
                };
                Some(RestoreHeader::new(p))
            }
            None => None,
        }
    }
}
