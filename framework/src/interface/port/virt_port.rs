use super::super::{PacketRx, PacketTx};
use super::PortStats;
use allocators::*;
use common::*;
use native::zcsi::{mbuf_alloc_bulk, mbuf_free_bulk, MBuf};
use std::fmt;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct VirtualPort {
    stats_rx: Arc<CacheAligned<PortStats>>,
    stats_tx: Arc<CacheAligned<PortStats>>,
}

#[derive(Clone)]
pub struct VirtualQueue {
    stats_rx: Arc<CacheAligned<PortStats>>,
    stats_tx: Arc<CacheAligned<PortStats>>,
}

impl fmt::Display for VirtualQueue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "virtual queue")
    }
}

impl PacketTx for VirtualQueue {
    #[inline]
    fn send(&mut self, pkts: &mut [*mut MBuf]) -> errors::Result<u32> {
        let len = pkts.len() as i32;
        let update = self.stats_tx.stats.load(Ordering::Relaxed) + len as usize;
        self.stats_tx.stats.store(update, Ordering::Relaxed);
        unsafe {
            mbuf_free_bulk(pkts.as_mut_ptr(), len);
        }
        Ok(len as u32)
    }
}

impl PacketRx for VirtualQueue {
    /// Send a batch of packets out this PortQueue. Note this method is internal to NetBricks (should not be directly
    /// called).
    #[inline]
    fn recv(&self, pkts: &mut [*mut MBuf]) -> errors::Result<(u32, i32)> {
        let len = pkts.len() as i32;
        let status = unsafe { mbuf_alloc_bulk(pkts.as_mut_ptr(), len as u32) };
        let alloced = if status == 0 { len } else { 0 };
        let update = self.stats_rx.stats.load(Ordering::Relaxed) + alloced as usize;
        self.stats_rx.stats.store(update, Ordering::Relaxed);
        Ok((alloced as u32, 0))
    }

    #[inline]
    fn queued(&self) -> usize {
        1
    }
}

impl VirtualPort {
    pub fn new() -> errors::Result<Arc<VirtualPort>> {
        Ok(Arc::new(VirtualPort {
            stats_rx: Arc::new(PortStats::new()),
            stats_tx: Arc::new(PortStats::new()),
        }))
    }

    pub fn new_virtual_queue(&self) -> errors::Result<CacheAligned<VirtualQueue>> {
        Ok(CacheAligned::allocate(VirtualQueue {
            stats_rx: self.stats_rx.clone(),
            stats_tx: self.stats_tx.clone(),
        }))
    }

    /// Get stats for an RX/TX queue pair.
    pub fn stats(&self) -> (usize, usize) {
        (
            self.stats_rx.stats.load(Ordering::Relaxed),
            self.stats_tx.stats.load(Ordering::Relaxed),
        )
    }
}
