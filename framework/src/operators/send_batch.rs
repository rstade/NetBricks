use super::act::Act;
use super::iterator::*;
use super::packet_batch::PacketBatch;
use super::Batch;
use common::*;
use interface::{PacketTx, Pdu};
use scheduler::Executable;

pub struct SendBatch<Port, V>
where
    Port: PacketTx,
    V: Batch + BatchIterator + Act,
{
    port: Port,
    parent: V,
}

impl<Port, V> SendBatch<Port, V>
where
    Port: PacketTx,
    V: Batch + BatchIterator + Act,
{
    pub fn new(parent: V, port: Port) -> SendBatch<Port, V> {
        SendBatch {
            port: port,
            parent: parent,
        }
    }
}

impl<Port, V> Batch for SendBatch<Port, V>
where
    Port: PacketTx,
    V: Batch + BatchIterator + Act,
{
    #[inline]
    fn queued(&self) -> usize {
        self.parent.queued()
    }
}

impl<Port, V> BatchIterator for SendBatch<Port, V>
where
    Port: PacketTx,
    V: Batch + BatchIterator + Act,
{
    #[inline]
    fn start(&mut self) -> usize {
        panic!("Cannot iterate send batch")
    }

    #[inline]
    fn next_payload(&mut self, _: usize) -> Option<Pdu> {
        panic!("Cannot iterate send batch")
    }
}

/// Internal interface for packets.
impl<Port, V> Act for SendBatch<Port, V>
where
    Port: PacketTx,
    V: Batch + BatchIterator + Act,
{
    #[inline]
    fn act(&mut self) -> (u32, i32) {
        // debug!("SendBatch.act with port {}", self.port.port_id());
        // First everything is applied
        let mut count: u32 = 0;
        let pre = self.parent.act();
        self.parent
            .get_packet_batch()
            .send_q(&mut self.port)
            .and_then(|x| {
                count = x;
                //                self.sent += x as u64;
                Ok(x)
            })
            .expect("Send failed");
        self.parent.done();
        (count, pre.1)
    }

    fn done(&mut self) {}

    fn send_q(&mut self, _: &mut dyn PacketTx) -> errors::Result<u32> {
        panic!("Cannot send a sent packet batch")
    }

    fn capacity(&self) -> i32 {
        self.parent.capacity()
    }

    #[inline]
    fn drop_packets(&mut self, _: &[usize]) -> Option<usize> {
        panic!("Cannot drop packets from a sent batch")
    }

    #[inline]
    fn drop_packets_all(&mut self) -> Option<usize> {
        panic!("Cannot drop packets from a sent batch")
    }

    #[inline]
    fn clear_packets(&mut self) {
        panic!("Cannot clear packets from a sent batch")
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

impl<Port, V> Executable for SendBatch<Port, V>
where
    Port: PacketTx,
    V: Batch + BatchIterator + Act,
{
    #[inline]
    fn execute(&mut self) -> (u32, i32) {
        self.act()
    }

    //    #[inline]
    //    fn dependencies(&mut self) -> Vec<usize> {
    //        self.get_task_dependencies()
    //    }
}
