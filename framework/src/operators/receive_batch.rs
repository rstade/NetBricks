use super::act::Act;
use super::iterator::*;
use super::packet_batch::PacketBatch;
use super::Batch;
use common::*;
use headers::NullHeader;
use interface::{PacketRx, PacketTx};

pub struct ReceiveBatch<T: PacketRx> {
    parent: PacketBatch,
    packet_rx: T,
    pub received: u64,
}

impl<T: PacketRx> ReceiveBatch<T> {
    pub fn new_with_parent(parent: PacketBatch, packet_rx: T) -> ReceiveBatch<T> {
        ReceiveBatch {
            parent,
            packet_rx,
            received: 0,
        }
    }

    pub fn new(packet_rx: T) -> ReceiveBatch<T> {
        ReceiveBatch {
            parent: PacketBatch::new(32, false),
            packet_rx,
            received: 0,
        }
    }

    pub fn new_keep_mbuf(packet_rx: T) -> ReceiveBatch<T> {
        ReceiveBatch {
            parent: PacketBatch::new(32, true),
            packet_rx,
            received: 0,
        }
    }
}

impl<T: PacketRx> Batch for ReceiveBatch<T> {}

impl<T: PacketRx> BatchIterator for ReceiveBatch<T> {
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
impl<T: PacketRx> Act for ReceiveBatch<T> {
    #[inline]
    fn act(&mut self) -> u32 {
        let mut count = 0;
        self.parent.act();
        self.parent
            .recv(&self.packet_rx)
            .and_then(|x| {
                /*
                if x > 0 && self.packet_rx.port_id().is_some() {
                    trace!(
                        "received batch with {} packets on port {}. ",
                        x,
                        self.packet_rx.port_id().unwrap()
                    );
                }
*/
                self.received += x as u64;
                count = x;
                Ok(x)
            }).expect("Receive failure");
        count
    }

    #[inline]
    fn done(&mut self) {
        // Free up memory
        self.parent.deallocate_batch().expect("Deallocation failed");
    }

    #[inline]
    fn send_q(&mut self, port: &PacketTx) -> errors::Result<u32> {
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
    fn clear_packets(&mut self) {
        self.parent.clear_packets()
    }

    #[inline]
    fn get_packet_batch(&mut self) -> &mut PacketBatch {
        &mut self.parent
    }

    //    #[inline]
    //    fn get_task_dependencies(&self) -> Vec<usize> {
    //        self.parent.get_task_dependencies()
    //    }
}
