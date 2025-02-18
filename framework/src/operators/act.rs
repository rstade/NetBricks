use super::packet_batch::PacketBatch;
use common::*;
use interface::PacketTx;

pub trait Act {
    /// Actually perform whatever needs to be done by this processing node.
    fn act(&mut self) -> (u32, i32); // returns (processed packets, queue length (if >= 0)

    /// Notification indicating we are done processing the current batch of packets
    fn done(&mut self);

    fn send_q(&mut self, port: &mut dyn PacketTx) -> errors::Result<u32>;

    fn capacity(&self) -> i32;

    fn drop_packets(&mut self, idxes: &[usize]) -> Option<usize>;

    fn drop_packets_all(&mut self) -> Option<usize>;

    /// Remove all packets from the batch (without actually freeing them).
    fn clear_packets(&mut self) {
        self.get_packet_batch().clear_packets();
    }

    fn get_packet_batch(&mut self) -> &mut PacketBatch;

    //    /// Get tasks that feed produce packets for this batch. We use this in the embedded scheduler.
    //    #[inline]
    //    fn get_task_dependencies(&self) -> Vec<usize>;
}
