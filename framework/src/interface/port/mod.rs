pub use self::fdir::*;
pub use self::phy_port::*;
pub use self::virt_port::*;

use allocators::*;
use common::*;
use interface::{PacketRx, PacketTx};
use native::zcsi::MBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub mod fdir;
mod phy_port;
mod virt_port;

/// Statistics for PMD port.
pub struct PortStats {
    pub stats: AtomicUsize,
    pub queued: AtomicUsize,
    pub q_len: AtomicUsize,
    pub max_q_len: AtomicUsize,
    pub cycles: AtomicU64,
}

impl PortStats {
    pub fn new() -> CacheAligned<PortStats> {
        CacheAligned::allocate(PortStats {
            stats: AtomicUsize::new(0),
            queued: AtomicUsize::new(0),
            q_len: AtomicUsize::new(0),
            max_q_len: AtomicUsize::new(0),
            cycles: AtomicU64::new(0),
        })
    }

    pub fn get_q_len(&self) -> usize {
        self.q_len.load(Ordering::Relaxed)
    }
    pub fn get_max_q_len(&self) -> usize {
        self.max_q_len.load(Ordering::Relaxed)
    }
    pub fn cycles(&self) -> u64 {
        self.cycles.load(Ordering::Relaxed)
    }

    pub fn set_q_len(&self, len: usize) -> usize {
        let q_max = self.get_max_q_len();
        if len > q_max {
            self.max_q_len.store(len, Ordering::Relaxed);
        }
        self.q_len.swap(len, Ordering::Relaxed)
    }
}

impl<T: PacketRx> PacketRx for CacheAligned<T> {
    #[inline]
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)> {
        T::recv(&*self, pkts)
    }
}

impl<T: PacketTx> PacketTx for CacheAligned<T> {
    #[inline]
    fn send(&mut self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        T::send(&mut *self, pkts)
    }
}
