pub use self::phy_port::*;
pub use self::virt_port::*;
pub use self::fdir::*;

use allocators::*;
use common::*;
use interface::{PacketRx, PacketTx};
use native::zcsi::MBuf;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};

mod phy_port;
mod virt_port;
pub mod fdir;

/// Statistics for PMD port.
pub struct PortStats {
    pub stats: AtomicUsize,
    pub q_len: AtomicUsize,
    pub max_q_len: AtomicUsize,
    pub cycles: AtomicU64,
}

impl PortStats {
    pub fn new() -> CacheAligned<PortStats> {
        CacheAligned::allocate(PortStats {
            stats: AtomicUsize::new(0),
            q_len: AtomicUsize::new(0),
            max_q_len: AtomicUsize::new(0),
            cycles: AtomicU64::new(0),
        })
    }

    pub fn get_q_len(&self) -> usize { self.q_len.load(Ordering::Relaxed) }
    pub fn get_max_q_len(&self) -> usize { self.max_q_len.load(Ordering::Relaxed) }
    pub fn cycles(&self) -> u64 { self.cycles.load(Ordering::Relaxed) }

    pub fn set_q_len(&self, len: usize) -> usize {
        let q_max= self.get_max_q_len();
        if len > q_max { self.max_q_len.store(len, Ordering::Relaxed);}
        self.q_len.swap(len, Ordering::Relaxed)
    }

}

impl<T: PacketRx> PacketRx for CacheAligned<T> {
    #[inline]
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)> {
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
