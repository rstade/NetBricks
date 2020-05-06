use std::cmp;
use std::fmt;
use std::mem;
use std::ops::Range;
use std::ptr;
use std::slice;

use common::errors;
use common::errors::ErrorKind;
use headers::{ArpIpv4Header, EndOffset, Header, IpHeader, MacHeader, TcpHeader, UdpHeader};
use native::zcsi::{mbuf_alloc, mbuf_alloc_bulk, validate_tx_offload, MBuf};
use utils::ipv4_checksum;

const MAX_HEADERS: usize = 5;

#[derive(Clone, Debug)]
pub struct HeaderStack<'a> {
    stack: [Header<'a>; MAX_HEADERS],
    /// header count
    hc: usize,
}

/// there are no bound checks!
impl<'a> HeaderStack<'a> {
    #[inline]
    pub fn new() -> HeaderStack<'a> {
        HeaderStack {
            stack: [Header::Null, Header::Null, Header::Null, Header::Null, Header::Null],
            hc: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, h: Header<'a>) {
        self.stack[self.hc] = h;
        self.hc += 1;
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.hc
    }

    #[inline]
    pub fn get(&self, which: usize) -> &Header<'a> {
        &self.stack[which]
    }

    #[inline]
    pub fn get_mut(&mut self, which: usize) -> &mut Header<'a> {
        &mut self.stack[which]
    }

    #[inline]
    pub fn get_slice(&self, range: Range<usize>) -> &[Header<'a>] {
        &self.stack[range]
    }

    #[inline]
    pub fn tcp_mut(&mut self, which: usize) -> &mut TcpHeader {
        self.stack[which].as_tcp_mut().unwrap()
    }

    #[inline]
    pub fn ip_mut(&mut self, which: usize) -> &mut IpHeader {
        self.stack[which].as_ip_mut().unwrap()
    }

    #[inline]
    pub fn mac_mut(&mut self, which: usize) -> &mut MacHeader {
        self.stack[which].as_mac_mut().unwrap()
    }

    #[inline]
    pub fn arp_mut(&mut self, which: usize) -> &mut ArpIpv4Header {
        self.stack[which].as_arpipv4_mut().unwrap()
    }

    #[inline]
    pub fn tcp(&self, which: usize) -> &TcpHeader {
        self.stack[which].as_tcp().unwrap()
    }

    #[inline]
    pub fn ip(&self, which: usize) -> &IpHeader {
        self.stack[which].as_ip().unwrap()
    }

    #[inline]
    pub fn mac(&self, which: usize) -> &MacHeader {
        self.stack[which].as_mac().unwrap()
    }

    #[inline]
    pub fn arp(&self, which: usize) -> &ArpIpv4Header {
        self.stack[which].as_arpipv4().unwrap()
    }
}

impl<'a> fmt::Display for HeaderStack<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut r = Ok(());
        if self.hc == 0 {
            r = write!(f, "<no headers>");
        } else {
            for i in 0..self.hc {
                r = writeln!(f, "{:1}: {}", i, self.stack[i as usize]);
                r?
            }
        }
        r
    }
}

#[repr(align(16))]
pub struct Pdu<'a> {
    header_stack: HeaderStack<'a>,
    mbuf: *mut MBuf,
}

impl<'a> fmt::Display for Pdu<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "({}, data_len= {}), headers=\n{ }",
            unsafe { &*self.mbuf },
            self.data_len(),
            self.header_stack,
        )
    }
}

impl<'a> Pdu<'a> {
    /// Allocate a new pdu.
    #[inline]
    pub fn new_pdu() -> Option<Pdu<'a>> {
        unsafe {
            // This sets refcnt = 1
            let mbuf = mbuf_alloc();
            if mbuf.is_null() {
                None
            } else {
                Some(Pdu {
                    mbuf,
                    header_stack: HeaderStack::new(),
                })
            }
        }
    }

    /// Allocate an array of pdus.
    pub fn new_pdu_array() -> Option<Vec<Pdu<'static>>> {
        let mut pkts = [ptr::null_mut::<MBuf>(); 32];
        unsafe {
            let alloc_ret = mbuf_alloc_bulk(pkts.as_mut_ptr(), pkts.len() as u32);
            if alloc_ret == 0 {
                Some(pkts.iter().map(|m| Pdu::pdu_from_mbuf_no_increment(*m)).collect())
            } else {
                None
            }
        }
    }

    #[inline]
    pub fn pdu_from_mbuf(mbuf: *mut MBuf) -> Pdu<'a> {
        // Need to up the refcnt, so that things don't drop.
        reference_mbuf(mbuf);
        Pdu::pdu_from_mbuf_no_increment(mbuf)
    }

    #[inline]
    pub fn pdu_from_mbuf_no_increment(mbuf: *mut MBuf) -> Pdu<'a> {
        let mut pdu = Pdu {
            mbuf,
            header_stack: HeaderStack::new(),
        };
        pdu.parse();
        pdu
    }

    #[inline]
    pub fn refcnt(&self) -> u16 {
        unsafe { (*self.mbuf).refcnt() }
    }

    #[inline]
    pub fn dereference_mbuf(&mut self) -> u16 {
        unsafe {
            (*self.mbuf).dereference();
        }
        self.refcnt()
    }

    #[inline]
    pub unsafe fn copy_use_mbuf(&self, mbuf: *mut MBuf) -> Pdu {
        assert!(!mbuf.is_null());
        (*self.mbuf).copy_to(mbuf.as_mut().unwrap());
        Pdu::pdu_from_mbuf_no_increment(mbuf)
    }

    /// copy gets us a new mbuf
    #[inline]
    pub unsafe fn copy(&self) -> Pdu {
        // This sets refcnt = 1
        let mbuf = mbuf_alloc();
        self.copy_use_mbuf(mbuf)
    }

    /// clone has same mbuf as the original and increments mbuf ref count
    /// clone replicates the mutable references to the headers, therefore it is unsafe, see parse()
    #[inline]
    pub fn clone(&mut self) -> Pdu<'static> {
        Pdu::pdu_from_mbuf(self.mbuf)
    }

    /// same as clone, but without increment of mbuf ref count
    #[inline]
    pub fn clone_without_ref_counting(&mut self) -> Pdu {
        Pdu::pdu_from_mbuf_no_increment(self.mbuf)
    }

    #[inline]
    pub fn add_padding(&mut self, nbytes: usize) -> usize {
        self.increase_payload_size(nbytes)
    }

    #[inline]
    fn parse_tcp(&mut self, offset: usize) {
        let hdr = unsafe { (*self.mbuf).data_address(offset) as *mut TcpHeader };
        unsafe {
            self.header_stack.push(Header::Tcp(&mut *hdr));
        }
    }

    #[inline]
    fn parse_ipv4(&mut self, offset: usize) {
        let hdr = unsafe { (*self.mbuf).data_address(offset) as *mut IpHeader };
        unsafe {
            self.header_stack.push(Header::Ip(&mut *hdr));
        }
        let ip_length;
        let ip_protocol;
        let ip_offset;
        unsafe {
            let ip = &mut *hdr;
            ip_length = ip.length();
            ip_protocol = ip.protocol();
            ip_offset = ip.offset();
        }
        match ip_protocol {
            6 => {
                if self.data_len() >= ip_length as usize + offset {
                    self.parse_tcp(offset + ip_offset);
                }
            }
            _ => {}
        }
    }

    #[inline]
    fn parse_arp(&mut self, offset: usize) {
        //TODO generalize for any protocol type, not only Ipv4
        let hdr = unsafe { (*self.mbuf).data_address(offset) as *mut ArpIpv4Header };

        unsafe {
            let arp = &mut *hdr;
            if arp.hw_type() == 1 && arp.proto_etype() == 0x0800 {
                self.header_stack.push(Header::ArpIpv4(&mut *hdr));
            }
        }
    }

    /// assumes an Ethernet frame and parses the frame up to Layer 4 if possible
    #[inline]
    pub fn parse(&mut self) -> usize {
        let l = self.data_len();
        if l < MacHeader::size() {
            return 0;
        };
        let hdr = unsafe { (*self.mbuf).data_address(0) as *mut MacHeader };
        unsafe {
            self.header_stack.push(Header::Mac(&mut *hdr));
        }
        let mac = unsafe { *hdr };
        match mac.etype() {
            0x8100 => {
                warn!("received 802.1Q frame");
            }
            0x9100 => {
                warn!("received 802.1AD frame");
            }
            //private etype packets are IP packets:
            0x0800 | 0x08FE | 0x08FF => {
                if l >= mac.offset() + IpHeader::size() {
                    self.parse_ipv4(mac.offset());
                }
            }
            0x86DD => {} // IPv6
            0x0806 => {
                if l >= mac.offset() + ArpIpv4Header::size() {
                    self.parse_arp(mac.offset());
                }
            } // ARP
            e => warn!("received Ethertype {:x}", e),
        }
        self.header_stack.count()
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

    // this includes ethernet padding if it is present, sta
    #[inline]
    pub fn data_len(&self) -> usize {
        unsafe { (*self.mbuf).data_len() }
    }

    #[inline]
    pub fn get_tailroom(&self) -> usize {
        unsafe { (*self.mbuf).pkt_tailroom() }
    }

    #[inline]
    pub fn headers(&self) -> &HeaderStack<'a> {
        &self.header_stack
    }

    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderStack<'a> {
        &mut self.header_stack
    }

    #[inline]
    pub fn replace_header(&mut self, which: usize, hdr: &Header) {
        unsafe {
            let pdu_header = self.header_stack.get_mut(which);
            assert_eq!(hdr.kind(), pdu_header.kind());

            match *pdu_header {
                Header::Null => (),
                Header::Mac(ref mut p) => ptr::copy_nonoverlapping(hdr.as_mac().unwrap() as *const MacHeader, *p, 1),
                Header::Ip(ref mut p) => ptr::copy_nonoverlapping(hdr.as_ip().unwrap() as *const IpHeader, *p, 1),
                Header::Tcp(ref mut p) => ptr::copy_nonoverlapping(hdr.as_tcp().unwrap() as *const TcpHeader, *p, 1),
                Header::Udp(ref mut p) => ptr::copy_nonoverlapping(hdr.as_udp().unwrap() as *const UdpHeader, *p, 1),
                Header::ArpIpv4(ref mut p) => {
                    ptr::copy_nonoverlapping(hdr.as_arpipv4().unwrap() as *const ArpIpv4Header, *p, 1)
                }
            };
        }
    }

    pub unsafe fn replace(&mut self, other: Pdu<'static>) -> Pdu {
        mem::replace(self, other)
    }

    /// Append a header to the header stack of a packet
    pub fn push_header<T: EndOffset>(&mut self, header: &T) -> bool {
        let size = header.offset();
        let added = unsafe { (*self.mbuf).add_data_end(size) };
        if added < size {
            return false;
        };
        let hdr = header as *const T;
        if self.header_stack.count() == 0 {
            let payload_sz = self.data_len();
            unsafe {
                let dst = if payload_sz > 0 {
                    // Need to move the payload down.
                    let final_dst = (*self.mbuf).data_address(0);
                    let move_loc = final_dst.offset(size as isize);
                    ptr::copy_nonoverlapping(final_dst, move_loc, payload_sz);
                    final_dst as *mut T
                } else {
                    (*self.mbuf).data_address(0) as *mut T
                };
                ptr::copy_nonoverlapping(hdr, dst, 1);
                self.header_stack.push(Header::new(dst));
                true
            }
        } else {
            let last_header_ix = self.header_stack.count() - 1;
            let payload_sz = self.payload_size(last_header_ix);
            unsafe {
                let dst = if payload_sz > 0 {
                    // Need to move the payload down.
                    let final_dst = self.payload_mut(last_header_ix).unwrap();
                    let move_loc = final_dst.offset(size as isize);
                    ptr::copy_nonoverlapping(final_dst, move_loc, payload_sz);
                    final_dst as *mut T
                } else {
                    self.payload(last_header_ix).unwrap() as *mut T
                };
                ptr::copy_nonoverlapping(hdr, dst, 1);
                self.header_stack.push(Header::new(dst));
                true
            }
        }
    }

    #[inline]
    pub fn set_tcp_ipv4_checksum_tx_offload(&mut self) {
        unsafe {
            (*self.mbuf).set_tcp_ipv4_checksum_tx_offload();
        }
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
        unsafe {
            (*self.mbuf).set_l2_len(val);
        }
    }

    #[inline]
    pub fn l3_len(&self) -> u64 {
        unsafe { (*self.mbuf).l3_len() }
    }

    #[inline]
    pub fn set_l3_len(&mut self, val: u64) {
        unsafe {
            (*self.mbuf).set_l3_len(val);
        }
    }

    #[inline]
    pub fn l4_len(&self) -> u64 {
        unsafe { (*self.mbuf).l4_len() }
    }

    #[inline]
    pub fn set_l4_len(&mut self, val: u64) {
        unsafe {
            (*self.mbuf).set_l4_len(val);
        }
    }

    #[inline]
    pub fn ol_flags(&self) -> u64 {
        unsafe { (*self.mbuf).ol_flags }
    }

    #[inline]
    pub fn clear_offload_flags(&mut self) {
        unsafe { (*self.mbuf).clear_offload_flags() }
    }

    #[inline]
    pub fn clear_rx_offload_flags(&mut self) -> u64 {
        unsafe { (*self.mbuf).clear_rx_offload_flags() }
    }
    /// returns 0 if no problem found
    #[inline]
    pub fn validate_tx_offload(&self) -> i32 {
        unsafe { validate_tx_offload(self.mbuf) }
    }

    #[inline]
    pub fn trim_payload_size(&mut self, trim_by: usize) -> usize {
        unsafe { (*self.mbuf).remove_data_end(trim_by) }
    }

    #[inline]
    pub fn increase_payload_size(&mut self, increase_by: usize) -> usize {
        unsafe { (*self.mbuf).add_data_end(increase_by) }
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
    fn payload_mut(&mut self, which: usize) -> Option<*mut u8> {
        let headers = self.header_stack.count();
        match which {
            x if x + 1 < headers => self.header_stack.get_mut(x + 1).as_ptr_u8_mut(),
            x if x == headers - 1 => Some(unsafe {
                self.header_stack
                    .get_mut(x)
                    .as_ptr_u8_mut()
                    .unwrap()
                    .offset(self.header_stack.get(x).offset().unwrap() as isize)
            }),
            _ => None,
        }
    }

    #[inline]
    fn payload(&self, which: usize) -> Option<*const u8> {
        let headers = self.header_stack.count();
        match which {
            x if x + 1 < headers => self.header_stack.get(x + 1).as_ptr_u8(),
            x if x == headers - 1 => Some(unsafe {
                self.header_stack
                    .get(x)
                    .as_ptr_u8()
                    .unwrap()
                    .offset(self.header_stack.get(x).offset().unwrap() as isize)
            }),
            _ => None,
        }
    }

    #[inline]
    pub fn get_payload(&self, which: usize) -> &[u8] {
        unsafe {
            let len = self.payload_size(which);
            slice::from_raw_parts(self.payload(which).unwrap(), len)
        }
    }

    #[inline]
    pub fn get_payload_mut(&mut self, which: usize) -> &mut [u8] {
        unsafe {
            let len = self.payload_size(which);
            slice::from_raw_parts_mut(self.payload_mut(which).unwrap(), len)
        }
    }

    /// may include padding
    #[inline]
    pub fn payload_size(&self, which: usize) -> usize {
        // sum up the header offsets
        let sum = self
            .header_stack
            .get_slice(0..which as usize + 1)
            .iter()
            .fold(0, |sum, value| sum + value.offset().unwrap());
        self.data_len() - sum
    }

    #[inline]
    pub fn copy_payload_from_u8_slice(&mut self, payload: &[u8], which: usize) -> usize {
        let copy_len = payload.len();
        if copy_len > 0 {
            let dst = self.payload_mut(which).unwrap();
            let src = payload.as_ptr();
            let payload_size = self.payload_size(which);
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

    /// fills the end of the payload with <len> bytes of value <byte>
    #[inline]
    pub fn write_from_tail_down(&mut self, len: usize, byte: u8) -> usize {
        let payload_size = self.payload_size(self.header_stack.count() - 1);
        if payload_size > 0 {
            let count = cmp::min(payload_size, len);
            let headers = self.header_stack.count();
            let dst = unsafe {
                self.payload_mut(headers - 1)
                    .unwrap()
                    .offset(payload_size as isize - count as isize)
            };
            unsafe {
                ptr::write_bytes(dst, byte, count);
            }
            count
        } else {
            0
        }
    }

    #[inline]
    pub fn port_id(&self) -> u16 {
        unsafe { (*self.mbuf).port }
    }
}

#[inline]
fn reference_mbuf(mbuf: *mut MBuf) {
    unsafe { (*mbuf).reference() };
}

#[inline]
pub fn update_tcp_checksum_(tcp_header: &mut TcpHeader, ip_payload_size: usize, ip_src: u32, ip_dst: u32) {
    let chk;
    {
        chk = ipv4_checksum(
            tcp_header as *mut TcpHeader as *mut u8,
            ip_payload_size,
            8,
            &[],
            ip_src,
            ip_dst,
            6u32,
        );
    }
    tcp_header.set_checksum(chk);
}
