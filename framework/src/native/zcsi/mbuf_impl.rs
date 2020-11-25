use native::zcsi::rte_mbuf_api::{rte_mbuf, PKT_TX_IPV4, PKT_TX_IP_CKSUM, PKT_TX_TCP_CKSUM};
use std::fmt;
use std::ptr;

pub type MBuf = rte_mbuf;

/* this must be adapted when new RX offloads are added */
pub const PKT_RX_OFFLOAD_MASK: u32 = (1 << 20) - 1;

#[allow(dead_code)]
impl MBuf {
    #[inline]
    pub fn read_metadata_slot(mbuf: *mut MBuf, slot: usize) -> usize {
        unsafe {
            let ptr = (mbuf.offset(1) as *mut usize).offset(slot as isize);
            *ptr
        }
    }

    #[inline]
    pub fn write_metadata_slot(mbuf: *mut MBuf, slot: usize, value: usize) {
        unsafe {
            let ptr = (mbuf.offset(1) as *mut usize).offset(slot as isize);
            *ptr = value;
        }
    }

    #[inline]
    pub unsafe fn metadata_as<T: Sized>(mbuf: *const MBuf, slot: usize) -> *const T {
        (mbuf.offset(1) as *const usize).offset(slot as isize) as *const T
    }

    #[inline]
    pub unsafe fn mut_metadata_as<T: Sized>(mbuf: *mut MBuf, slot: usize) -> *mut T {
        (mbuf.offset(1) as *mut usize).offset(slot as isize) as *mut T
    }

    #[inline]
    pub fn data_address(&self, offset: usize) -> *mut u8 {
        unsafe { (self.buf_addr as *mut u8).offset(self.data_off as isize + offset as isize) }
    }

    /// Returns the total allocated size of this mbuf segment.
    /// This is a constant.
    #[inline]
    pub fn buf_len(&self) -> usize {
        self.buf_len as usize
    }

    /// Returns the length of data in this mbuf segment.
    #[inline]
    pub fn data_len(&self) -> usize {
        self.data_len as usize
    }

    /// Returns the size of the packet (across multiple mbuf segment).
    #[inline]
    pub fn pkt_len(&self) -> usize {
        self.pkt_len as usize
    }

    #[inline]
    fn pkt_headroom(&self) -> usize {
        self.data_off as usize
    }

    #[inline]
    pub fn pkt_tailroom(&self) -> usize {
        self.buf_len() - self.data_off as usize - self.data_len()
    }

    /// Add data to the beginning of the packet. This might fail (i.e., return 0) when no more headroom is left.
    #[inline]
    pub fn add_data_beginning(&mut self, len: usize) -> usize {
        // If only we could add a likely here.
        if len > self.pkt_headroom() {
            0
        } else {
            self.data_off -= len as u16;
            self.data_len += len as u16;
            self.pkt_len += len as u32;
            len
        }
    }

    /// Add data to the end of a packet buffer. This might fail (i.e., return 0) when no more tailroom is left. We do
    /// not currently deal with packet with multiple segments.
    #[inline]
    pub fn add_data_end(&mut self, len: usize) -> usize {
        if len > self.pkt_tailroom() {
            0
        } else {
            self.data_len += len as u16;
            self.pkt_len += len as u32;
            len
        }
    }

    #[inline]
    pub fn remove_data_beginning(&mut self, len: usize) -> usize {
        if len > self.data_len() {
            0
        } else {
            self.data_off += len as u16;
            self.data_len -= len as u16;
            self.pkt_len -= len as u32;
            len
        }
    }

    #[inline]
    pub fn remove_data_end(&mut self, len: usize) -> usize {
        if len > self.data_len() {
            0
        } else {
            self.data_len -= len as u16;
            self.pkt_len -= len as u32;
            len
        }
    }

    #[inline]
    pub fn refcnt(&self) -> u16 {
        self.refcnt
    }

    #[inline]
    pub fn reference(&mut self) {
        self.refcnt += 1;
    }

    #[inline]
    pub fn dereference(&mut self) {
        self.refcnt -= 1;
    }

    #[inline]
    pub fn set_refcnt(&mut self, new_value: u16) {
        self.refcnt = new_value;
    }

    // copy payload and selected fields to target tmb
    #[inline]
    pub fn copy_to(&self, tmb: &mut MBuf) {
        (*tmb).data_len = (*self).data_len;
        (*tmb).data_off = (*self).data_off;
        (*tmb).pkt_len = (*self).pkt_len;
        unsafe {
            ptr::copy_nonoverlapping(self.data_address(0), (*tmb).data_address(0), self.data_len());
        }
    }

    #[inline]
    pub fn clear_offload_flags(&mut self) {
        self.ol_flags = 0;
    }

    #[inline]
    pub fn clear_rx_offload_flags(&mut self) -> u64 {
        self.ol_flags &= !(PKT_RX_OFFLOAD_MASK as u64);
        self.ol_flags
    }

    #[inline]
    pub fn set_tcp_ipv4_checksum_tx_offload(&mut self) {
        self.ol_flags |= PKT_TX_IPV4 | PKT_TX_IP_CKSUM | PKT_TX_TCP_CKSUM;
    }

    #[inline]
    pub fn ipv4_checksum_tx_offload(&mut self) -> bool {
        self.ol_flags & PKT_TX_IPV4 != 0 && self.ol_flags & PKT_TX_IP_CKSUM != 0
    }

    #[inline]
    pub fn tcp_checksum_tx_offload(&mut self) -> bool {
        self.ol_flags & PKT_TX_TCP_CKSUM != 0
    }

    #[inline]
    pub fn l2_len(&self) -> u64 {
        unsafe { self.__bindgen_anon_3.__bindgen_anon_1.l2_len() }
    }
    #[inline]
    pub fn set_l2_len(&mut self, val: u64) {
        unsafe {
            self.__bindgen_anon_3.__bindgen_anon_1.set_l2_len(val);
        }
    }

    #[inline]
    pub fn l3_len(&self) -> u64 {
        unsafe { self.__bindgen_anon_3.__bindgen_anon_1.l3_len() }
    }

    #[inline]
    pub fn set_l3_len(&mut self, val: u64) {
        unsafe {
            self.__bindgen_anon_3.__bindgen_anon_1.set_l3_len(val);
        }
    }

    #[inline]
    pub fn l4_len(&self) -> u64 {
        unsafe { self.__bindgen_anon_3.__bindgen_anon_1.l4_len() }
    }

    #[inline]
    pub fn set_l4_len(&mut self, val: u64) {
        unsafe {
            self.__bindgen_anon_3.__bindgen_anon_1.set_l4_len(val);
        }
    }
}

impl fmt::Display for MBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MBuf(&buf_addr= {:p}, data_len= {}, refcnt= {}, data_off= {}, data=",
            self.buf_addr,
            self.data_len(),
            self.refcnt(),
            self.data_off,
        )?;
        for i in 0..self.data_len {
            write!(f, "{:x}", unsafe { *self.data_address(i as usize) })?;
        }
        Ok(())
    }
}
