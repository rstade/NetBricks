pub use self::filter_batch::FilterBatch;
pub use self::drop::DropBatch;
pub use self::group_by::*;
pub use self::map_batch::MapBatch;
pub use self::merge_batch::MergeBatchTraitObj;
pub use self::merge_batch::MergeBatch;
pub use self::merge_batch_auto::MergeBatchAuto;
pub use self::receive_batch::ReceiveBatch;
pub use self::send_batch::SendBatch;
pub use self::transform_batch::TransformBatch;
pub use self::iterator::BatchIterator;
pub use self::act::Act;
pub use self::packet_batch::PacketBatch;
pub use self::composition_batch::CompositionBatch;
use self::transform_batch::TransformFn;
use self::map_batch::MapFn;
use self::filter_batch::FilterFn;

use interface::*;
use scheduler::Scheduler;
use uuid::Uuid;

#[macro_use]
mod macros;
mod act;
mod filter_batch;
mod group_by;
mod iterator;
mod map_batch;
mod merge_batch;
mod merge_batch_auto;
mod packet_batch;
mod receive_batch;
mod send_batch;
mod transform_batch;
mod drop;
mod composition_batch;

/// Merge a vector of batches into one batch. Currently this just round-robins between merged batches, but in the future
/// the precise batch being processed will be determined by the scheduling policy used.

pub enum SchedulingPolicy {
    RoundRobin,
    LongestQueue,
}

#[inline]
pub fn merge_batches(batches: Vec<Box<Batch>>) -> MergeBatchTraitObj {
    MergeBatchTraitObj::new(batches)
}

#[inline]
pub fn merge<T: Batch>(batches: Vec<T>) -> MergeBatch<T> {
    MergeBatch::new(batches)
}

#[inline]
pub fn merge_with_selector(batches: Vec<Box<Batch>>, selector: Vec<usize>) -> MergeBatchTraitObj {
    MergeBatchTraitObj::new_with_selector(batches, selector)
}

#[inline]
pub fn merge_auto(batches: Vec<Box<Batch>>, policy: SchedulingPolicy) -> MergeBatchAuto {
    MergeBatchAuto::new(batches, policy)
}

/// Public trait implemented by every packet batch type. This trait should be used as a constraint for any functions or
/// places where a Batch type is required.
///
pub trait Batch: BatchIterator + Act {
    fn queued(&self) -> usize;

    /// Send this batch out a particular port and queue.
    fn send<Port: PacketTx>(self, port: Port) -> SendBatch<Port, Self>
    where
        Self: Sized,
    {
        SendBatch::<Port, Self>::new(self, port)
    }

    /// Transform a header field.
    fn transform(self, transformer: TransformFn) -> TransformBatch<Self>
    where
        Self: Sized,
    {
        TransformBatch::<Self>::new(self, transformer)
    }

    /// Map over a set of header fields. Map and transform primarily differ in map being immutable. Immutability
    /// provides some optimization opportunities not otherwise available.
    fn map(self, transformer: MapFn) -> MapBatch<Self>
    where
        Self: Sized,
    {
        MapBatch::<Self>::new(self, transformer)
    }

    /// Filter out packets, any packets for which `filter_f` returns false are dropped from the batch.
    fn filter(self, filter_f: FilterFn) -> FilterBatch<Self>
    where
        Self: Sized,
    {
        FilterBatch::<Self>::new(self, filter_f)
    }

    fn drop(self) -> DropBatch<Self>
    where Self: Sized, {  DropBatch::<Self>::new(self) }

    fn group_by<S: Scheduler + Sized>(
        self,
        groups: usize,
        // group_f: GroupFn<Self::Header, Self::Metadata>,
        group_f: GroupFnPdu,
        sched: &mut S,
        name: String,
        uuid: Uuid,
    ) -> GroupBy<Self>
    where
        Self: Sized,
    {
        GroupBy::<Self>::new(self, groups, group_f, sched, name, uuid)
    }

    fn compose(self) -> CompositionBatch
        where
            Self: Sized + 'static,
    {
        CompositionBatch::new(self)
    }
}
