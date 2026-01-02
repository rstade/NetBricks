use std::sync::Arc;
use crate::allocators::CacheAligned;
pub use self::pdu::*;
pub use self::port::*;
pub mod dpdk;
mod pciaddress;
mod pdu;
mod port;

use crate::common::errors;
use crate::native::zcsi::MBuf;

/// Generic trait for objects that can receive packets.
pub trait PacketRx {
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)>; // (packets received, queue length (if >=0))
    #[inline]
    fn rx_stats(&self) -> Arc<CacheAligned<PortStats>>;
    fn queued(&self) -> usize;
}

/// Generic trait for objects that can send packets.
pub trait PacketTx {
    fn send(&mut self, pkts: &mut [*mut MBuf]) -> errors::Result<u32>;
    #[inline]
    fn tx_stats(&self) -> Arc<CacheAligned<PortStats>>;
}

pub trait PacketRxTx: PacketRx + PacketTx {}
