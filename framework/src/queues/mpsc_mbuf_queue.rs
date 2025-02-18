use common::*;
use interface::{PacketRx, Pdu};
use native::zcsi::MBuf;
use operators::ReceiveBatch;
use std::arch::x86_64::_mm_pause;
use std::clone::Clone;
use std::cmp::min;
use std::default::Default;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;
use utils::round_to_power_of_2;

#[derive(Default)]
struct QueueMetadata {
    pub head: AtomicUsize,
    pub tail: AtomicUsize,
}

/// A multiproducer single consumer queue for mbufs. The main difference when compared to `std::sync::mpsc` is that this
/// does not use a linked list (to avoid allocation). The hope is to eventually turn this into something that can carry
/// `Packets` or sufficient metadata to reconstruct that structure.
struct MpscQueue {
    slots: usize, // Must be a power of 2
    mask: usize,  // slots - 1
    // TODO: Watermark?
    producer: QueueMetadata,
    consumer: QueueMetadata,
    queue: Vec<AtomicPtr<MBuf>>,
    n_producers: AtomicUsize, // Number of producers.
}

impl MpscQueue {
    pub fn new(size: usize) -> MpscQueue {
        let slots = if size & (size - 1) != 0 {
            round_to_power_of_2(size)
        } else {
            size
        };
        MpscQueue {
            slots: slots,
            mask: slots - 1,
            queue: (0..slots).map(|_| Default::default()).collect(),
            producer: Default::default(),
            consumer: Default::default(),
            n_producers: Default::default(),
        }
    }

    // This assumes that no producers are currently active.
    #[inline]
    pub fn reference_producers(&self) {
        self.n_producers.fetch_add(1, Ordering::AcqRel);
    }

    #[inline]
    fn enqueue_mbufs(&self, start: usize, enqueue: usize, mbufs: &[*mut MBuf]) {
        let mask = self.mask;
        let len = self.slots;
        let mut mbuf_idx = 0;
        let mut queue_idx = start & mask;
        // TODO: Unroll?
        if queue_idx + enqueue >= len {
            while queue_idx < len {
                self.queue[queue_idx].store(mbufs[mbuf_idx], Ordering::Release);
                mbuf_idx += 1;
                queue_idx += 1;
            }
            queue_idx = 0;
        }
        while mbuf_idx < enqueue {
            self.queue[queue_idx].store(mbufs[mbuf_idx], Ordering::Release);
            mbuf_idx += 1;
            queue_idx += 1;
        }
    }

    #[inline]
    pub fn enqueue(&self, mbufs: &[*mut MBuf]) -> usize {
        let producers = self.n_producers.load(Ordering::Acquire);
        assert!(producers >= 1, "Insertion into a queue without producers");
        if producers == 1 {
            self.enqueue_sp(mbufs)
        } else {
            self.enqueue_mp(mbufs)
        }
    }

    // In the mp only version lots of time was being consumed in CAS (Compare and Swap). We want to allow for the mp case, but there is no
    // need to waste cycles.
    #[inline]
    fn enqueue_sp(&self, mbufs: &[*mut MBuf]) -> usize {
        let len = mbufs.len();

        let producer_head = self.producer.head.load(Ordering::Acquire);
        let consumer_tail = self.consumer.tail.load(Ordering::Acquire);

        let free = self.mask.wrapping_add(consumer_tail).wrapping_sub(producer_head);
        let insert = min(free, len);

        if insert > 0 {
            let producer_next = producer_head.wrapping_add(insert);
            // Reserve slots by incrementing head
            self.producer.head.store(producer_next, Ordering::Release);
            // Write to reserved slot.
            self.enqueue_mbufs(producer_head, insert, &mbufs[..insert]);
            // Commit write by changing tail.
            // Once this has been achieved, update tail. Any conflicting updates will wait on the previous spin lock.
            self.producer.tail.store(producer_next, Ordering::Release);
            insert
        } else {
            0
        }
    }

    #[inline]
    fn free_slots(&self) -> usize {
        let producer_head = self.producer.head.load(Ordering::Acquire);
        let consumer_tail = self.consumer.tail.load(Ordering::Acquire);
        self.mask.wrapping_add(consumer_tail).wrapping_sub(producer_head)
    }

    #[inline]
    fn used_slots(&self) -> usize {
        self.slots - self.free_slots() - 1
    }

    #[inline]
    fn enqueue_mp(&self, mbufs: &[*mut MBuf]) -> usize {
        let len = mbufs.len();
        let mut insert;
        let mut producer_head;
        let mut consumer_tail;
        // First try and reserve memory by incrementing producer head.
        while {
            producer_head = self.producer.head.load(Ordering::Acquire);
            consumer_tail = self.consumer.tail.load(Ordering::Acquire);
            let free = self.mask.wrapping_add(consumer_tail).wrapping_sub(producer_head);
            insert = min(free, len);
            if insert == 0 {
                // Short circuit, no insertion
                false // This is the same as break in this construct.
            } else {
                let producer_next = producer_head.wrapping_add(insert);
                self.producer
                    .head
                    .compare_exchange(producer_head, producer_next, Ordering::AcqRel, Ordering::Relaxed)
                    .is_err()
            }
        } {}

        if insert > 0 {
            // If we successfully reserved memory, write to memory.
            let end = producer_head.wrapping_add(insert);
            self.enqueue_mbufs(producer_head, insert, &mbufs[..insert]);
            // Commit write by changing tail.
            // Before committing we wait for any preceding writes to finish (this is important since we assume buffer is
            // always available upto commit point.
            while {
                let producer_tail = self.producer.tail.load(Ordering::Acquire);
                producer_tail != producer_head
            } {
                unsafe {
                    _mm_pause();
                } // Pausing is a nice thing to do during spin locks
            }
            // Once this has been achieved, update tail. Any conflicting updates will wait on the previous spin lock.
            self.producer.tail.store(end, Ordering::Release);
            insert
        } else {
            0
        }
    }

    #[inline]
    pub fn enqueue_one(&self, mbuf: *mut MBuf) -> bool {
        self.enqueue(&[mbuf]) == 1
    }

    #[inline]
    fn dequeue_mbufs(&self, start: usize, dequeue: usize, mbufs: &mut [*mut MBuf]) {
        let mask = self.mask;
        let len = self.slots;
        // TODO: Unroll?
        let mut mbuf_idx = 0;
        let mut queue_idx = start & mask;
        if queue_idx + dequeue >= len {
            while queue_idx < len {
                mbufs[mbuf_idx] = self.queue[queue_idx].load(Ordering::Acquire);
                mbuf_idx += 1;
                queue_idx += 1;
            }
            queue_idx = 0;
        }
        while mbuf_idx < dequeue {
            mbufs[mbuf_idx] = self.queue[queue_idx].load(Ordering::Acquire);
            mbuf_idx += 1;
            queue_idx += 1;
        }
    }

    #[inline]
    pub fn dequeue(&self, mbufs: &mut [*mut MBuf]) -> usize {
        // NOTE: This is a single consumer dequeue as assumed by this queue.
        let consumer_head = self.consumer.head.load(Ordering::Acquire);
        let producer_tail = self.producer.tail.load(Ordering::Acquire);
        let available_entries = producer_tail.wrapping_sub(consumer_head);
        let dequeue = min(mbufs.len(), available_entries);
        if dequeue > 0 {
            let consumer_next = consumer_head.wrapping_add(dequeue);
            // Reserve what we are going to dequeue.
            self.consumer.head.store(consumer_next, Ordering::Release);
            self.dequeue_mbufs(consumer_head, dequeue, mbufs);
            // Commit that we have dequeued.
            self.consumer.tail.store(consumer_next, Ordering::Release);
        }
        dequeue
    }
}

pub struct MpscProducer {
    mpsc_queue: Arc<MpscQueue>,
}

// Need an explicit clone mechanism so that we can reference as appropriate
impl Clone for MpscProducer {
    fn clone(&self) -> MpscProducer {
        let q = self.mpsc_queue.clone();
        q.reference_producers();
        MpscProducer { mpsc_queue: q }
    }
}

impl MpscProducer {
    pub fn enqueue(&self, pdus: &mut Vec<Pdu>) -> usize {
        let mbufs: Vec<_> = pdus.drain(..).map(|p| unsafe { p.get_mbuf() }).collect();
        self.mpsc_queue.enqueue(&mbufs[..])
    }

    #[inline]
    pub fn enqueue_mbufs(&self, mbufs: &[*mut MBuf]) -> usize {
        self.mpsc_queue.enqueue(mbufs)
    }

    #[inline]
    pub fn enqueue_one(&self, pdu: Pdu) -> bool {
        unsafe { self.mpsc_queue.enqueue_one(pdu.get_mbuf()) }
    }

    #[inline]
    pub fn enqueue_one_boxed(&self, pdu: Box<Pdu>) -> bool {
        unsafe { self.mpsc_queue.enqueue_one(pdu.get_mbuf()) }
    }

    #[inline]
    pub fn free_slots(&self) -> usize {
        self.mpsc_queue.free_slots()
    }

    #[inline]
    pub fn used_slots(&self) -> usize {
        self.mpsc_queue.used_slots()
    }
}

pub struct MpscConsumer {
    mpsc_queue: Arc<MpscQueue>,
}

impl PacketRx for MpscConsumer {
    #[inline]
    fn recv(&self, mbufs: &mut [*mut MBuf]) -> errors::Result<(u32, i32)> {
        Ok((
            self.mpsc_queue.dequeue(mbufs) as u32,
            self.mpsc_queue.used_slots() as i32,
        ))
    }

    #[inline]
    fn queued(&self) -> usize {
        self.mpsc_queue.used_slots()
    }
}

pub fn new_mpsc_queue_pair_with_size(size: usize) -> (MpscProducer, ReceiveBatch<MpscConsumer>) {
    let mpsc_q = Arc::new(MpscQueue::new(size));
    mpsc_q.reference_producers();
    (
        MpscProducer {
            mpsc_queue: mpsc_q.clone(),
        },
        ReceiveBatch::new(MpscConsumer { mpsc_queue: mpsc_q }),
    )
}

const DEFAULT_QUEUE_SIZE: usize = 1024;

pub fn new_mpsc_queue_pair() -> (MpscProducer, ReceiveBatch<MpscConsumer>) {
    new_mpsc_queue_pair_with_size(DEFAULT_QUEUE_SIZE)
}
