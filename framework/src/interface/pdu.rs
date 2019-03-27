use std::cmp;
use std::fmt;
use std::ptr;
use std::slice;

use common::errors;
use common::errors::ErrorKind;
use headers::{EndOffset, Header, IpHeader, MacHeader, NullHeader, TcpHeader};
use native::zcsi::{validate_tx_offload, MBuf};
use utils::ipv4_checksum;

pub struct Pdu {
    mbuf: *mut MBuf,
    headers: Vec<Header>,
}

impl fmt::Display for Pdu {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(&mbuf={:p}, {}, data_len= {}), headers={:?}",
            self.mbuf,
            unsafe { &*self.mbuf },
            self.data_len(),
            self.headers,
        )
    }
}

impl Pdu {
    #[inline]
    fn parse_tcp(&mut self, offset: usize) {
        let hdr = unsafe { (*self.mbuf).data_address(offset) as *mut TcpHeader };
        self.headers.push(Header::Tcp(hdr));
    }

    #[inline]
    fn parse_ipv4(&mut self, offset: usize) {
        let hdr = unsafe { (*self.mbuf).data_address(offset) as *mut IpHeader };
        self.headers.push(Header::Ip(hdr));
        let ip = unsafe { *hdr };
        match ip.protocol() {
            6 => {
                if self.data_len() >= ip.length() as usize + offset {
                    self.parse_tcp(offset + ip.offset());
                }
            }
            _ => {}
        }
    }

    /// assumes an Ethernet frame and parses the frame up to Layer 4 if possible
    pub fn parse(&mut self) -> usize {
        let l = self.data_len();
        if l < MacHeader::size() {
            return 0;
        };
        let hdr = unsafe { (*self.mbuf).data_address(0) as *mut MacHeader };
        self.headers.push(Header::Mac(hdr));
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
            0x0806 => {} // ARP
            e => warn!("received Ethertype {:x}", e),
        }
        self.headers.len()
    }

    // this includes ethernet padding if it is present, sta
    #[inline]
    pub fn data_len(&self) -> usize {
        unsafe { (*self.mbuf).data_len() }
    }

    /// same as clone, but without increment of mbuf ref count
    #[inline]
    pub fn clone_without_ref_counting(&mut self) -> Pdu {
        Pdu {
            mbuf: self.mbuf,
            headers: self.headers.clone(),
        }
    }

    #[inline]
    pub fn get_header(&self, which: usize) -> Option<&Header> {
        self.headers.get(which)
    }

    #[inline]
    pub fn get_header_mut(&mut self, which: usize) -> Option<&mut Header> {
        self.headers.get_mut(which)
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
    fn payload(&self, which: usize) -> Option<*mut u8> {
        let headers = self.headers.len();
        match which {
            x if x + 1 < headers => self.headers[x + 1].as_ptr_u8(),
            x if x == headers - 1 => Some(unsafe {
                self.headers[x]
                    .as_ptr_u8()
                    .unwrap()
                    .offset(self.headers[x].offset().unwrap() as isize)
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

    /// may include padding
    #[inline]
    pub fn payload_size(&self, which: usize) -> usize {
        // sum up the header offsets
        let sum = self.headers[0..which]
            .iter()
            .fold(0, |sum, value| sum + value.offset().unwrap());
        self.data_len() - sum
    }

    #[inline]
    pub fn copy_payload_from_u8_slice(&mut self, payload: &[u8], which: usize) -> usize {
        let copy_len = payload.len();
        if copy_len > 0 {
            let dst = self.payload(which).unwrap();
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
        let payload_size = self.payload_size(self.headers.len() - 1);
        if payload_size > 0 {
            let count = cmp::min(payload_size, len);
            let dst = unsafe {
                self.payload(self.headers.len() - 1)
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
}

#[inline]
fn reference_mbuf(mbuf: *mut MBuf) {
    unsafe { (*mbuf).reference() };
}

#[inline]
pub unsafe fn pdu_from_mbuf(mbuf: *mut MBuf) -> Pdu {
    // Need to up the refcnt, so that things don't drop.
    reference_mbuf(mbuf);
    pdu_from_mbuf_no_increment(mbuf)
}

#[inline]
pub unsafe fn pdu_from_mbuf_no_increment(mbuf: *mut MBuf) -> Pdu {
    let mut pdu = Pdu {
        mbuf,
        headers: Vec::with_capacity(5),
    };
    pdu.parse();
    pdu
}

#[inline]
pub fn update_tcp_checksum_(tcp_header: *mut TcpHeader, ip_payload_size: usize, ip_src: u32, ip_dst: u32) {
    let chk;
    {
        chk = ipv4_checksum(tcp_header as *mut u8, ip_payload_size, 8, &[], ip_src, ip_dst, 6u32);
    }
    unsafe { *tcp_header }.set_checksum(chk);
}
