use std::marker::PhantomData;
use super::act::Act;
use super::Batch;
use super::iterator::BatchIterator;
use super::packet_batch::cast_from_u8;
use super::super::interface::EndOffset;
use super::super::pmd::*;
use super::super::interface::Result;

pub struct ParsedBatch<T: EndOffset, V>
    where V: Batch + BatchIterator + Act
{
    parent: V,
    phantom: PhantomData<T>,
}

impl<T, V> Act for ParsedBatch<T, V>
    where T: EndOffset,
          V: Batch + BatchIterator + Act
{
    fn act(&mut self) -> &mut Self {
        self.parent.act();
        self
    }

    fn done(&mut self) -> &mut Self {
        self.parent.done();
        self
    }

    fn send_queue(&mut self, port: &mut PmdPort, queue: i32) -> Result<u32> {
        self.parent.send_queue(port, queue)
    }

    fn capacity(&self) -> i32 {
        self.parent.capacity()
    }
}

batch!{ParsedBatch, [parent: V], [phantom: PhantomData]}

impl<T, V> BatchIterator for ParsedBatch<T, V>
    where T: EndOffset,
          V: Batch + BatchIterator + Act
{
    #[inline]
    fn start(&mut self) -> usize {
        self.parent.start()
    }

    #[inline]
    unsafe fn payload(&mut self, idx: usize) -> *mut u8 {
        let address = self.parent.payload(idx);
        let offset = T::offset(cast_from_u8::<T>(address));
        address.offset(offset as isize)
    }

    #[inline]
    unsafe fn address(&mut self, idx: usize) -> *mut u8 {
        self.parent.payload(idx)
    }

    #[inline]
    unsafe fn next_address(&mut self, idx: usize) -> Option<(*mut u8, usize)> {
        self.parent.next_payload(idx)
    }

    #[inline]
    unsafe fn next_payload(&mut self, idx: usize) -> Option<(*mut u8, usize)> {
        let parent_payload = self.parent.next_payload(idx);
        match parent_payload {
            Some((packet, idx)) => {
                let offset = T::offset(cast_from_u8::<T>(packet));
                Some((packet.offset(offset as isize), idx))
            }
            None => None,
        }
    }

    #[inline]
    unsafe fn base_address(&mut self, idx: usize) -> *mut u8 {
        self.parent.base_address(idx)
    }

    #[inline]
    unsafe fn base_payload(&mut self, idx: usize) -> *mut u8 {
        self.parent.base_payload(idx)
    }

    #[inline]
    unsafe fn next_base_address(&mut self, idx: usize) -> Option<(*mut u8, usize)> {
        self.parent.next_base_address(idx)
    }

    #[inline]
    unsafe fn next_base_payload(&mut self, idx: usize) -> Option<(*mut u8, usize)> {
        self.parent.next_base_payload(idx)
    }
}