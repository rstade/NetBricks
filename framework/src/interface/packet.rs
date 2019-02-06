use common::EmptyMetadata;
use common::errors;
use common::errors::ErrorKind;
use headers::{EndOffset, NullHeader, TcpHeader};
use native::zcsi::*;
use std::fmt;
use std::marker::PhantomData;
use std::mem::size_of;
use std::ptr;
use std::cmp;
use std::slice;
use std::option::Option;
use utils::ipv4_checksum;
//use std::option::Option::{None, Some};
use std::marker::Sized;
use std::prelude::v1::*;
use std::mem;

/// A packet is a safe wrapper around mbufs, that can be allocated and manipulated.
/// We associate a header type with a packet to allow safe insertion of headers.

unsafe impl<T: EndOffset, M: Sized + Send> Send for Packet<T, M> {}

pub struct Packet<T: EndOffset, M: Sized + Send> {
    mbuf: *mut MBuf,
    _phantom_m: PhantomData<M>,
    pre_pre_header: Option<*mut <<T as EndOffset>::PreviousHeader as EndOffset>::PreviousHeader>,
    pre_header: Option<*mut T::PreviousHeader>,
    header: *mut T,
    offset: usize,
}

impl<T: EndOffset, M: Sized + Send> fmt::Display for Packet<T, M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(&mbuf={:p}, {}, payload_size= {}, offset={}, header= {:?}, pre_header= {:?}, pre_pre_header= {:?})",
            self.mbuf,
            unsafe { &*self.mbuf },
            self.payload_size(),
            self.offset,
            self.header as *mut T,
            self.pre_header,
            self.pre_pre_header,
        )
    }
}

#[inline]
unsafe fn create_packet<T: EndOffset, M: Sized + Send>(mbuf: *mut MBuf, hdr: *mut T, offset: usize) -> Packet<T, M> {
    let pkt = Packet::<T, M> {
        mbuf: mbuf,
        _phantom_m: PhantomData,
        offset,
        pre_pre_header: None,
        pre_header: None,
        header: hdr,
    };
    pkt
}

#[inline]
unsafe fn create_follow_packet<T: EndOffset, M: Sized + Send>(
    mut p: Packet<T::PreviousHeader, M>,
    hdr: *mut T,
    offset: usize,
) -> Packet<T, M> {
    let pkt = Packet::<T, M> {
        mbuf: p.get_mbuf_ref(),
        _phantom_m: PhantomData,
        offset,
        header: hdr,
        pre_pre_header: p.pre_header,
        pre_header: Some(p.header),
    };
    pkt
}

fn reference_mbuf(mbuf: *mut MBuf) {
    unsafe { (*mbuf).reference() };
}

pub const METADATA_SLOTS: u16 = 16;
const HEADER_SLOT: usize = 0;
const OFFSET_SLOT: usize = HEADER_SLOT + 1;
const STACK_DEPTH_SLOT: usize = OFFSET_SLOT + 1;
const STACK_OFFSET_SLOT: usize = STACK_DEPTH_SLOT + 1;
const STACK_SIZE: usize = 0;
#[allow(dead_code)]
const END_OF_STACK_SLOT: usize = STACK_OFFSET_SLOT + STACK_SIZE;
const FREEFORM_METADATA_SLOT: usize = END_OF_STACK_SLOT;
const FREEFORM_METADATA_SIZE: usize = (METADATA_SLOTS as usize - FREEFORM_METADATA_SLOT) * 8;

#[inline]
pub unsafe fn packet_from_mbuf<T: EndOffset>(mbuf: *mut MBuf, offset: usize) -> Packet<T, EmptyMetadata> {
    // Need to up the refcnt, so that things don't drop.
    reference_mbuf(mbuf);
    packet_from_mbuf_no_increment(mbuf, offset)
}

#[inline]
pub unsafe fn packet_from_mbuf_no_increment<T: EndOffset>(mbuf: *mut MBuf, offset: usize) -> Packet<T, EmptyMetadata> {
    // Compute the real offset
    let header = (*mbuf).data_address(offset) as *mut T;
    create_packet(mbuf, header, offset)
}

#[inline]
pub unsafe fn packet_from_mbuf_no_free<T: EndOffset>(mbuf: *mut MBuf, offset: usize) -> Packet<T, EmptyMetadata> {
    packet_from_mbuf_no_increment(mbuf, offset)
}

/// Allocate a new packet.
pub fn new_packet() -> Option<Packet<NullHeader, EmptyMetadata>> {
    unsafe {
        // This sets refcnt = 1
        let mbuf = mbuf_alloc();
        if mbuf.is_null() {
            None
        } else {
            Some(packet_from_mbuf_no_increment(mbuf, 0))
        }
    }
}

/// Allocate an array of packets.
pub fn new_packet_array(pkts: &mut [*mut MBuf]) -> Vec<Packet<NullHeader, EmptyMetadata>> {
    //let mut array = Vec::with_capacity(count);
    unsafe {
        let alloc_ret = mbuf_alloc_bulk(pkts.as_mut_ptr(), pkts.len() as u32);
        if alloc_ret == 0 {
            //            array.set_len(count);
        }
        pkts.iter().map(|m| packet_from_mbuf_no_increment(*m, 0)).collect()
    }
}

impl<T: EndOffset, M: Sized + Send> Packet<T, M> {
    // --------------------- Not using packet offsets ------------------------------------------------------
    #[inline]
    fn header(&self) -> *mut T {
        self.header
    }

    #[inline]
    fn header_u8(&self) -> *mut u8 {
        self.header as *mut u8
    }

    #[inline]
    fn offset(&self) -> usize {
        self.offset
    }

    // -----------------Common code ------------------------------------------------------------------------
    #[inline]
    fn read_stack_depth(&self) -> usize {
        MBuf::read_metadata_slot(self.mbuf, STACK_DEPTH_SLOT)
    }

    #[inline]
    fn write_stack_depth(&mut self, new_depth: usize) {
        MBuf::write_metadata_slot(self.mbuf, STACK_DEPTH_SLOT, new_depth);
    }

    #[inline]
    fn read_stack_offset(&mut self, depth: usize) -> usize {
        MBuf::read_metadata_slot(self.mbuf, STACK_OFFSET_SLOT + depth)
    }

    #[inline]
    fn write_stack_offset(&mut self, depth: usize, offset: usize) {
        MBuf::write_metadata_slot(self.mbuf, STACK_OFFSET_SLOT + depth, offset)
    }

    #[inline]
    pub fn reset_stack_offset(&mut self) {
        self.write_stack_depth(0)
    }

    #[inline]
    #[cfg_attr(feature = "dev", allow(absurd_extreme_comparisons))]
    fn push_offset(&mut self, offset: usize) -> Option<usize> {
        let depth = self.read_stack_depth();
        if depth < STACK_SIZE {
            self.write_stack_depth(depth + 1);
            self.write_stack_offset(depth, offset);
            Some(depth + 1)
        } else {
            None
        }
    }

    #[inline]
    fn pop_offset(&mut self) -> Option<usize> {
        let depth = self.read_stack_depth();
        if depth > 0 {
            self.write_stack_depth(depth - 1);
            Some(self.read_stack_offset(depth - 1))
        } else {
            None
        }
    }

    #[inline]
    pub fn free_packet(self) {
        if !self.mbuf.is_null() {
            unsafe { mbuf_free(self.mbuf) };
        }
    }

    #[inline]
    fn update_ptrs(&mut self, header: *mut u8, offset: usize) {
        MBuf::write_metadata_slot(self.mbuf, HEADER_SLOT, header as usize);
        MBuf::write_metadata_slot(self.mbuf, OFFSET_SLOT, offset as usize);
    }

    /// Save the header and offset into the MBuf. This is useful for later restoring this information.
    #[inline]
    pub fn save_header_and_offset(&mut self) {
        let header = self.header_u8();
        let offset = self.offset();
        self.update_ptrs(header, offset)
    }

    #[inline]
    fn read_header<T2: EndOffset>(&self) -> *mut T2 {
        MBuf::read_metadata_slot(self.mbuf, HEADER_SLOT) as *mut T2
    }

    #[inline]
    fn read_offset(&self) -> usize {
        MBuf::read_metadata_slot(self.mbuf, OFFSET_SLOT)
    }

    #[inline]
    fn payload(&self) -> *mut u8 {
        unsafe {
            let payload_offset = self.payload_offset();
            self.header_u8().offset(payload_offset as isize)
        }
    }

    /// Return the offset of the payload relative to the header.
    #[inline]
    fn payload_offset(&self) -> usize {
        unsafe { (*self.header()).offset() }
    }

    #[inline]
    fn data_base(&self) -> *mut u8 {
        unsafe { (*self.mbuf).data_address(0) }
    }

    /// clone has same mbuf as the original
    #[inline]
    pub fn clone(&mut self) -> Packet<T, M> {
        reference_mbuf(self.mbuf);

        Packet::<T, M> {
            mbuf: self.mbuf,
            _phantom_m: PhantomData,
            offset: self.offset,
            pre_pre_header: self.pre_pre_header,
            pre_header: self.pre_header,
            header: self.header,
        }
    }

    /// same as clone, but without increment of mbuf ref count
    #[inline]
    pub fn clone_without_ref_counting(&mut self) -> Packet<T, M> {
        Packet::<T, M> {
            mbuf: self.mbuf,
            _phantom_m: PhantomData,
            offset: self.offset,
            pre_pre_header: self.pre_pre_header,
            pre_header: self.pre_header,
            header: self.header,
        }
    }


    /// copy gets us a new mbuf
    #[inline]
    pub unsafe fn copy(&self) -> Packet<T, M> {
        // unsafe { packet_from_mbuf(self.mbuf, self.offset) };
        // This sets refcnt = 1
        let mbuf = mbuf_alloc();
        self.copy_use_mbuf(mbuf)
    }

    #[inline]
    pub unsafe fn copy_use_mbuf(&self, mbuf: *mut MBuf) -> Packet<T, M> {
        assert!(!mbuf.is_null());
        let u8_header = (*mbuf).data_address(self.offset);
        let header = u8_header as *mut T;
        (*self.mbuf).copy_to(mbuf.as_mut().unwrap());

        let pre_header = if self.pre_header.is_some() {
            Some(u8_header.
                offset(
                    (self.pre_header.unwrap() as *mut u8).offset_from(self.header as *mut u8)
                ) as *mut T::PreviousHeader)
        } else { None };

        let pre_pre_header = if self.pre_pre_header.is_some() {
            Some(u8_header.
                offset(
                    (self.pre_pre_header.unwrap() as *mut u8).offset_from(self.header as *mut u8)
                ) as *mut <<T as EndOffset>::PreviousHeader as EndOffset>::PreviousHeader)
        } else { None };

        Packet::<T, M> {
            mbuf,
            _phantom_m: PhantomData,
            offset: self.offset,
            pre_pre_header,
            pre_header,
            header,
        }
    }

    pub unsafe fn replace(&mut self, other: Packet<T, M>) -> Packet<T, M> {
        mem::replace(self, other)
    }

    #[inline]
    pub fn data_len(&self) -> usize {
        unsafe { (*self.mbuf).data_len() }
    } // this includes also any ethernet padding, sta

    // this includes any ethernet padding,
    // and therefore can be larger than the actual payload, sta
    #[inline]
    pub fn payload_size(&self) -> usize {
        self.data_len() - self.offset() - self.payload_offset()
    }

    #[inline]
    pub fn get_header(&self) -> &T {
        unsafe { &(*(self.header())) }
    }

    #[inline]
    pub fn get_mut_header(&mut self) -> &mut T {
        unsafe { &mut (*(self.header())) }
    }

    #[inline]
    pub fn get_pre_header(&self) -> Option<&T::PreviousHeader> {
        unsafe {
            if self.pre_header.is_some() {
                Some(&(*(self.pre_header.unwrap())))
            } else {
                None
            }
        }
    }

    #[inline]
    pub fn get_mut_pre_header(&mut self) -> Option<&mut T::PreviousHeader> {
        unsafe {
            if self.pre_header.is_some() {
                Some(&mut (*(self.pre_header.unwrap())))
            } else {
                None
            }
        }
    }

    #[inline]
    pub fn get_pre_pre_header(&self) -> Option<&<<T as EndOffset>::PreviousHeader as EndOffset>::PreviousHeader> {
        unsafe {
            if self.pre_pre_header.is_some() {
                Some(&(*(self.pre_pre_header.unwrap())))
            } else {
                None
            }
        }
    }

    #[inline]
    pub fn get_mut_pre_pre_header(
        &mut self,
    ) -> Option<&mut <<T as EndOffset>::PreviousHeader as EndOffset>::PreviousHeader> {
        unsafe {
            if self.pre_pre_header.is_some() {
                Some(&mut (*(self.pre_pre_header.unwrap())))
            } else {
                None
            }
        }
    }

    #[inline]
    pub fn read_metadata(&self) -> &M {
        assert!(size_of::<M>() < FREEFORM_METADATA_SIZE);
        unsafe {
            let ptr = MBuf::metadata_as::<M>(self.mbuf, FREEFORM_METADATA_SLOT);
            &(*(ptr))
        }
    }

    #[inline]
    pub fn write_metadata<M2: Sized + Send>(&mut self, metadata: &M2) -> errors::Result<()> {
        if size_of::<M2>() >= FREEFORM_METADATA_SIZE {
            Err(ErrorKind::MetadataTooLarge.into())
        } else {
            unsafe {
                let ptr = MBuf::mut_metadata_as::<M2>(self.mbuf, FREEFORM_METADATA_SLOT);
                ptr::copy_nonoverlapping(metadata, ptr, 1);
                Ok(())
            }
        }
    }

    #[inline]
    pub fn reinterpret_metadata<M2: Sized + Send>(mut self) -> Packet<T, M2> {
        let hdr = self.header();
        let offset = self.offset();
        unsafe { create_packet(self.get_mbuf_ref(), hdr, offset) }
    }

    /// When constructing a packet, take a packet as input and add a header.
    #[inline]
    pub fn push_header<T2: EndOffset<PreviousHeader=T>>(self, header: &T2) -> Option<Packet<T2, M>> {
        unsafe {
            let len = self.data_len();
            let size = header.offset();
            let added = (*self.mbuf).add_data_end(size);

            let hdr = header as *const T2;
            let offset = self.offset() + self.payload_offset();
            if added >= size {
                let dst = if len != offset {
                    // Need to move down the rest of the data down.
                    let final_dst = self.payload();
                    let move_loc = final_dst.offset(size as isize);
                    let to_move = len - offset;
                    ptr::copy_nonoverlapping(final_dst, move_loc, to_move);
                    final_dst as *mut T2
                } else {
                    self.payload() as *mut T2
                };
                ptr::copy_nonoverlapping(hdr, dst, 1);
                Some(create_follow_packet(self, dst, offset))
            } else {
                None
            }
        }
    }

    /// Remove data at the top of the payload, useful when removing headers.
    #[inline]
    pub fn remove_from_payload_head(&mut self, size: usize) -> errors::Result<()> {
        unsafe {
            let src = self.data_base();
            let dst = src.offset(size as isize);
            ptr::copy_nonoverlapping(src, dst, size);
            (*self.mbuf).remove_data_beginning(size);
            Ok(())
        }
    }

    /// Add data to the head of the payload.
    #[inline]
    pub fn add_to_payload_head(&mut self, size: usize) -> errors::Result<()> {
        unsafe {
            let added = (*self.mbuf).add_data_end(size);
            if added >= size {
                let src = self.payload();
                let dst = src.offset(size as isize);
                ptr::copy_nonoverlapping(src, dst, size);
                Ok(())
            } else {
                Err(ErrorKind::FailedAllocation.into())
            }
        }
    }

    #[inline]
    pub fn remove_from_payload_tail(&mut self, size: usize) -> errors::Result<()> {
        unsafe {
            (*self.mbuf).remove_data_end(size);
            Ok(())
        }
    }

    #[inline]
    pub fn add_to_payload_tail(&mut self, size: usize) -> errors::Result<()> {
        unsafe {
            let added = (*self.mbuf).add_data_end(size);
            if added >= size {
                Ok(())
            } else {
                Err(ErrorKind::FailedAllocation.into())
            }
        }
    }

    #[inline]
    pub fn write_header<T2: EndOffset + Sized>(&mut self, header: &T2, offset: usize) -> errors::Result<()> {
        if offset > self.payload_size() {
            Err(ErrorKind::BadOffset(offset).into())
        } else {
            unsafe {
                let dst = self.payload().offset(offset as isize);
                ptr::copy_nonoverlapping(header, dst as *mut T2, 1);
            }
            Ok(())
        }
    }

    #[inline]
    pub fn parse_header<T2: EndOffset<PreviousHeader=T>>(self) -> Packet<T2, M> {
        let p_sz = self.payload_size();
        let t2_sz = T2::size();
        if p_sz < t2_sz { error!("payload sz= {}, T2::size= {}, {} {} {}", p_sz, t2_sz, self.data_len(), self.offset(), self.payload_offset()); assert! {p_sz >= t2_sz} }
        unsafe {
            let hdr = self.payload() as *mut T2;
            let offset = self.offset() + self.payload_offset();
            create_follow_packet(self, hdr, offset)
        }
    }

    #[inline]
    pub fn parse_header_and_record<T2: EndOffset<PreviousHeader=T>>(mut self) -> Packet<T2, M> {
        unsafe {
            assert! {self.payload_size() >= T2::size()}
            let hdr = self.payload() as *mut T2;
            let payload_offset = self.payload_offset();
            let offset = self.offset() + payload_offset;
            // TODO: Log failure?
            self.push_offset(payload_offset).unwrap();
            create_packet(self.get_mbuf_ref(), hdr, offset)
        }
    }

    #[inline]
    pub fn restore_saved_header<T2: EndOffset, M2: Sized + Send>(mut self) -> Option<Packet<T2, M2>> {
        unsafe {
            let hdr = self.read_header::<T2>();
            if hdr.is_null() {
                None
            } else {
                let offset = self.read_offset();
                Some(create_packet(self.get_mbuf_ref(), hdr, offset))
            }
        }
    }

    #[inline]
    pub fn replace_header(&mut self, hdr: &T) {
        unsafe {
            ptr::copy_nonoverlapping(hdr, self.header(), 1);
        }
    }

    #[inline]
    pub fn deparse_header(mut self, offset: usize) -> Packet<T::PreviousHeader, M> {
        let offset = offset as isize;
        unsafe {
            let header = self.header_u8().offset(-offset) as *mut T::PreviousHeader;
            let new_offset = self.offset() - offset as usize;
            create_packet(self.get_mbuf_ref(), header, new_offset)
        }
    }

    #[inline]
    pub fn deparse_header_stack(mut self) -> Option<Packet<T::PreviousHeader, M>> {
        self.pop_offset().map(|offset| self.deparse_header(offset))
    }

    #[inline]
    pub fn reset(mut self) -> Packet<NullHeader, EmptyMetadata> {
        unsafe {
            let header = self.data_base() as *mut NullHeader;
            create_packet(self.get_mbuf_ref(), header, 0)
        }
    }

    #[inline]
    pub fn get_mut_payload(&mut self) -> &mut [u8] {
        unsafe {
            let len = self.payload_size();
            let ptr = self.payload();
            slice::from_raw_parts_mut(ptr, len)
        }
    }

    #[inline]
    pub fn get_payload(&self) -> &[u8] {
        unsafe {
            let len = self.payload_size();
            slice::from_raw_parts(self.payload(), len)
        }
    }

    #[inline]
    pub fn get_payload_with_len(&self, len: usize) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self.payload(), len)
        }
    }


    #[inline]
    pub fn get_tailroom(&self) -> usize {
        unsafe { (*self.mbuf).pkt_tailroom() }
    }

    #[inline]
    pub fn increase_payload_size(&mut self, increase_by: usize) -> usize {
        unsafe { (*self.mbuf).add_data_end(increase_by) }
    }

    #[inline]
    pub fn trim_payload_size(&mut self, trim_by: usize) -> usize {
        unsafe { (*self.mbuf).remove_data_end(trim_by) }
    }

    #[inline]
    pub fn write_from_tail_down(&mut self, len: usize, byte: u8) -> usize {
        let payload_size=self.payload_size();
        if payload_size > 0 {
            let count= cmp::min(payload_size, len);
            let dst = unsafe { self.payload().offset(payload_size as isize - count as isize) };
            unsafe { ptr::write_bytes(dst, byte, count); }
            count
        } else { 0 }
    }

    #[inline]
    pub fn copy_payload_from<M2: Send + Sized>(&mut self, other: &Packet<T, M2>) -> usize {
        let copy_len = other.payload_size();
        let dst = self.payload();
        let src = other.payload();

        let payload_size = self.payload_size();

        let should_copy = if payload_size < copy_len {
            let increment = copy_len - payload_size;
            payload_size + self.increase_payload_size(increment)
        } else {
            copy_len
        };

        unsafe {
            ptr::copy_nonoverlapping(src, dst, should_copy);
            should_copy
        }
    }

    #[inline]
    pub fn copy_payload_to_bytearray(&mut self, bytearray: &mut Vec<u8>, size: usize) {
        let src = self.get_payload_with_len(size);
        unsafe { bytearray.set_len(size); }
        bytearray.as_mut_slice().copy_from_slice(src);
    }

    #[inline]
    pub fn copy_payload_from_bytearray(&mut self, payload: &Box<Vec<u8>>) -> usize {
        let copy_len = payload.len();
        if copy_len > 0 {
            let dst = self.payload();
            let src = payload.as_ptr();
            let payload_size = self.payload_size();
            let should_copy = if payload_size < copy_len {
                let increment = copy_len - payload_size;
                payload_size + self.increase_payload_size(increment)
            } else {
                copy_len
            };
            unsafe {
                ptr::copy_nonoverlapping(src, dst, should_copy);
                should_copy
            }
        } else {
            0usize
        }
    }

    #[inline]
    pub fn copy_payload_from_u8_slice(&mut self, payload: &[u8]) -> usize {
        let copy_len = payload.len();
        if copy_len > 0 {
            let dst = self.payload();
            let src = payload.as_ptr();
            let payload_size = self.payload_size();
            let should_copy = if payload_size < copy_len {
                let increment = copy_len - payload_size;
                payload_size + self.increase_payload_size(increment)
            } else {
                copy_len
            };
            unsafe {
                ptr::copy_nonoverlapping(src, dst, should_copy);
                should_copy
            }
        } else {
            0usize
        }
    }

    #[inline]
    pub fn add_padding(&mut self, nbytes: usize) -> usize {
        self.increase_payload_size(nbytes)
    }

    #[inline]
    pub fn refcnt(&self) -> u16 {
        unsafe { (*self.mbuf).refcnt() }
    }

    pub fn reference_mbuf(&mut self) -> u16 {
        reference_mbuf(self.mbuf);
        self.refcnt()
    }

    pub fn dereference_mbuf(&mut self) -> u16 {
        unsafe { (*self.mbuf).dereference(); }
        self.refcnt()
    }

    #[inline]
    pub fn set_refcnt(&self, refcnt: u16) {
        unsafe { (*self.mbuf).set_refcnt(refcnt) }
    }

    /// Get the mbuf reference by this packet.
    ///
    /// # Safety
    /// The reference held by this Packet is nulled out as a result of this code. The callee is responsible for
    /// appropriately freeing this mbuf from here-on out.
    #[inline]
    pub unsafe fn get_mbuf(mut self) -> *mut MBuf {
        self.get_mbuf_ref()
    }

    #[inline]
    unsafe fn get_mbuf_ref(&mut self) -> *mut MBuf {
        let mbuf = self.mbuf;
        self.mbuf = ptr::null_mut();
        mbuf
    }
    #[inline]
    pub fn set_tcp_ipv4_checksum_tx_offload(&mut self) {
        unsafe { (*self.mbuf).set_tcp_ipv4_checksum_tx_offload(); }
    }

    #[inline]
    pub fn ipv4_checksum_tx_offload(&self) -> bool {
        unsafe { (*self.mbuf).ipv4_checksum_tx_offload() }
    }

    #[inline]
    pub fn tcp_checksum_tx_offload(&self) -> bool {
        unsafe { (*self.mbuf).tcp_checksum_tx_offload() }
    }
    /// functions for tx offload
    #[inline]
    pub fn l2_len(&self) -> u64 {
        unsafe { (*self.mbuf).l2_len() }
    }
    #[inline]
    pub fn set_l2_len(&mut self, val: u64) {
        unsafe { (*self.mbuf).set_l2_len(val); }
    }

    #[inline]
    pub fn l3_len(&self) -> u64 {
        unsafe { (*self.mbuf).l3_len() }
    }
    #[inline]
    pub fn set_l3_len(&mut self, val: u64) {
        unsafe { (*self.mbuf).set_l3_len(val); }
    }

    #[inline]
    pub fn l4_len(&self) -> u64 {
        unsafe { (*self.mbuf).l4_len() }
    }
    #[inline]
    pub fn set_l4_len(&mut self, val: u64) {
        unsafe { (*self.mbuf).set_l4_len(val); }
    }
    #[inline]
    pub fn ol_flags(&self) -> u64 {
        unsafe { (*self.mbuf).ol_flags }
    }

    /// returns 0 if no problem found
    #[inline]
    pub fn validate_tx_offload(&self) -> i32 {
        unsafe { validate_tx_offload(self.mbuf) }
    }
}

#[inline]
pub fn update_tcp_checksum<M: Sized + Send>(
    p: &mut Packet<TcpHeader, M>,
    ip_payload_size: usize,
    ip_src: u32,
    ip_dst: u32,
) {
    let chk;
    {
        chk = ipv4_checksum(p.header as *mut u8, ip_payload_size, 8, &[], ip_src, ip_dst, 6u32);
    }
    p.get_mut_header().set_checksum(chk);
}


/* must run as root
#[cfg(test)]
mod tests {
use super::*;
use headers::IpHeader;
use headers::TcpHeader;
use headers::MacHeader;
use eui48::MacAddress;
use interface::dpdk::init_system_wl_with_mempool;

#[test]
fn packet_copy() {
    let name = String::from("packet_copy_test");


    init_system_wl_with_mempool(
        &name[..],
        1u64,
        0,
        &[],
        2048,
        32,
        &vec![],
    );

    let mut mac = MacHeader::new();
    mac.src = MacAddress::new([1; 6]);
    mac.set_etype(0x0800);
    let mut ip = IpHeader::new();
    ip.set_src(511);
    ip.set_ttl(128);
    ip.set_version(4);
    ip.set_protocol(6); //tcp
    ip.set_ihl(5);
    ip.set_length(40);
    ip.set_flags(0x2); // DF=1, MF=0 flag: don't fragment
    let mut tcp = TcpHeader::new();
    tcp.set_syn_flag();
    tcp.set_src_port(80);
    tcp.set_data_offset(5);

    let packet =
        new_packet()
            .unwrap()
            .push_header(&mac)
            .unwrap()
            .push_header(&ip)
            .unwrap()
            .push_header(&tcp)
            .unwrap();

    let copy;
    unsafe {
        copy = packet.copy();
    }

    //println!("original: {}", packet );
    //println!("copy: {}", copy );
    unsafe {
        assert_eq!(packet.header.as_ref().unwrap().src_port(), copy.header.as_ref().unwrap().src_port());
        assert_eq!(packet.pre_header.unwrap().as_ref().unwrap().src(), 511);
        assert_eq!(511, copy.pre_header.unwrap().as_ref().unwrap().src());
        assert_eq!(packet.pre_pre_header.unwrap().as_ref().unwrap().src, mac.src);
        assert_eq!(copy.pre_pre_header.unwrap().as_ref().unwrap().src, mac.src);
    }
}
}
*/