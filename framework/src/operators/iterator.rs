use interface::Pdu;
use std::cell::Cell;

/// An interface implemented by all batches for iterating through the set of packets in a batch.
/// This is private to the framework and not exposed.
///
/// # Safety
/// These methods return pointers to packet mbufs. As long as packet mbufs are treated
/// correctly (i.e., assumed freed after send, freed correctly, allocated correctly, etc.) this should be safe.
/// Furthermore, dropping a packet might result in unexpected behavior (e.g., packets being skipped) but will not result
/// in crashes. Generally, do not drop or move packets during iteration, it is safer to collect the list/set of
/// packets to be modified and apply this modification later. Everything about iterator invalidation is likely to change
/// later.
pub trait BatchIterator {
    /// Returns the starting index for the packet batch. This allows for cases where the head of the batch is not at
    /// index 0.
    fn start(&mut self) -> usize;

    fn next_payload(&mut self, idx: usize) -> Option<Pdu>;
}

/// A struct containing the parsed information returned by the `PayloadEnumerator`.
pub struct ParsedDescriptor<'a> {
    pub index: usize,
    pub pdu: Pdu<'a>,
}

/// An enumerator over both the header and the payload. The payload is represented as an appropriately sized slice of
/// bytes. The expectation is therefore that the user can operate on bytes, or make appropriate adjustments as
/// necessary.
pub struct PayloadEnumerator {
    // Was originally using a cell here so we didn't have to borrow the iterator mutably. I think at this point, given
    // that the batch is not stored in the iterator this might be a moot point, but it does allow the iterator to be
    // entirely immutable for the moment, which makes sense.
    idx: Cell<usize>,
}

impl<'a> PayloadEnumerator {
    /// Create a new iterator.
    #[inline]
    pub fn new(batch: &mut dyn BatchIterator) -> PayloadEnumerator {
        let start = batch.start();
        PayloadEnumerator { idx: Cell::new(start) }
    }

    /// Used for looping over packets. Note this iterator is not safe if packets are added or dropped during iteration,
    /// so you should not do that if possible.
    #[inline]
    pub fn next(&self, batch: &'a mut dyn BatchIterator) -> Option<ParsedDescriptor<'a>> {
        let original_idx = self.idx.get();
        let item = batch.next_payload(original_idx);
        match item {
            Some(pdu) => {
                // This is safe (assuming our size accounting has been correct so far).
                // Switch to providing packets
                self.idx.set(original_idx + 1);
                Some(ParsedDescriptor {
                    index: original_idx,
                    pdu,
                })
            }
            None => None,
        }
    }
}
