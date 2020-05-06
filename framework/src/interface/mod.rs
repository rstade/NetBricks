pub use self::pdu::*;
pub use self::port::*;
pub mod dpdk;
mod pdu;
mod port;
use common::errors;
use native::zcsi::MBuf;

/// Generic trait for objects that can receive packets.
pub trait PacketRx {
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)>; // (packets received, queue length (if >=0))
    fn queued(&self) -> usize;
}

/// Generic trait for objects that can send packets.
pub trait PacketTx {
    fn send(&mut self, pkts: &mut [*mut MBuf]) -> errors::Result<u32>;
}

pub trait PacketRxTx: PacketRx + PacketTx {}
