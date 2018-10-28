pub use self::phy_port::*;
pub use self::virt_port::*;
pub use self::fdir::*;

use allocators::*;
use common::*;
use interface::{PacketRx, PacketTx};
use native::zcsi::MBuf;
use std::sync::atomic::AtomicUsize;
mod phy_port;
mod virt_port;
pub mod fdir;

/// Statistics for PMD port.
struct PortStats {
    pub stats: AtomicUsize,
}

impl PortStats {
    pub fn new() -> CacheAligned<PortStats> {
        CacheAligned::allocate(PortStats {
            stats: AtomicUsize::new(0),
        })
    }
}

impl<T: PacketRx> PacketRx for CacheAligned<T> {
    #[inline]
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        T::recv(&*self, pkts)
    }

    fn port_id(&self) -> i32 {
        T::port_id(&*self)
    }
}

impl<T: PacketTx> PacketTx for CacheAligned<T> {
    #[inline]
    fn send(&self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        T::send(&*self, pkts)
    }

    fn port_id(&self) -> i32 {
        T::port_id(&*self)
    }
}
