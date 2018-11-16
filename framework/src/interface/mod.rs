pub use self::packet::*;
pub use self::port::*;
pub mod dpdk;
mod packet;
mod port;
use native::zcsi::MBuf;
use common::errors;

/// Generic trait for objects that can receive packets.
pub trait PacketRx: Send {
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)>; // (packets received, queue length (if >=0))
    fn port_id(&self) -> i32 {
        -1
    }
    fn queued(&self) -> usize { 1 }
}

/// Generic trait for objects that can send packets.
pub trait PacketTx: Send {
    fn send(&self, pkts: &mut [*mut MBuf]) -> errors::Result<u32>;
    fn port_id(&self) -> i32 {
        -1
    }
}

pub trait PacketRxTx: PacketRx + PacketTx {}
