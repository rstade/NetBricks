use super::act::Act;
use super::iterator::{BatchIterator, PacketDescriptor};
use super::packet_batch::PacketBatch;
use super::Batch;
use common::*;
use headers::EndOffset;
use headers::NullHeader;
use interface::PacketTx;
use scheduler::Executable;

/// `CompositionBatch` allows multiple NFs to be combined. A composition batch resets the packet pointer so that each NF
/// can treat packets as originating from the NF itself.
pub struct CompositionBatch {
    parent: Box<Batch<Header = NullHeader, Metadata = EmptyMetadata>>,
}

impl CompositionBatch {
    pub fn new<T: EndOffset, M: Sized + Send, V: 'static + Batch<Header = T, Metadata = M>>(
        parent: V,
    ) -> CompositionBatch {
        CompositionBatch {
            parent: box parent.reset(),
        }
    }
}

impl Batch for CompositionBatch {
    fn queued(&self) -> usize {
        self.parent.queued()
    }
}

impl BatchIterator for CompositionBatch {
    type Header = NullHeader;
    type Metadata = EmptyMetadata;

    #[inline]
    fn start(&mut self) -> usize {
        self.parent.start()
    }

    #[inline]
    unsafe fn next_payload(&mut self, idx: usize) -> Option<PacketDescriptor<NullHeader, EmptyMetadata>> {
        self.parent.next_payload(idx)
    }
}

/// Internal interface for packets.
impl Act for CompositionBatch {
    act! {}
}

impl Executable for CompositionBatch {
    #[inline]
    fn execute(&mut self) -> (u32, i32) {
        let count = self.act();
        self.done();
        count
    }
}
