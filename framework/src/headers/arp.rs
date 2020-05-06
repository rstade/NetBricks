use eui48::MacAddress;
use std::fmt;
use std::net::Ipv4Addr;

use super::{EndOffset, HeaderKind};

#[derive(Debug, Default, Clone, Copy)]
#[repr(C, packed)]
pub struct ArpIpv4Header {
    pub hw_type: u16,
    pub proto_etype: u16,
    pub hw_addr_len: u8,
    pub proto_addr_len: u8,
    pub operation: u16,
    pub sender_hw_addr: MacAddress,
    pub sender_proto_addr: u32,
    pub target_hw_addr: MacAddress,
    pub target_proto_addr: u32,
}

const HDR_SIZE: usize = 28;

impl EndOffset for ArpIpv4Header {
    #[inline]
    fn offset(&self) -> usize {
        2 * self.hw_addr_len as usize + 2 * self.proto_addr_len as usize + 8
    }
    #[inline]
    fn size() -> usize {
        HDR_SIZE
    }

    #[inline]
    fn payload_size(&self, hint: usize) -> usize {
        hint - self.offset()
    }

    #[inline]
    fn header_kind(&self) -> HeaderKind {
        HeaderKind::ArpIpv4
    }
}

impl fmt::Display for ArpIpv4Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Arp(hw_type= {}, proto_type= 0x{:04x}, op= {}, {} > {})",
            self.hw_type(),
            self.proto_etype(),
            self.operation(),
            self.sender_ip_addr(),
            self.target_ip_addr(),
        )
    }
}

impl ArpIpv4Header {
    #[inline]
    pub fn hw_type(&self) -> u16 {
        u16::from_be(self.hw_type)
    }
    #[inline]
    pub fn proto_etype(&self) -> u16 {
        u16::from_be(self.proto_etype)
    }
    #[inline]
    pub fn operation(&self) -> u16 {
        u16::from_be(self.operation)
    }
    #[inline]
    pub fn sender_proto_addr(&self) -> u32 {
        u32::from_be(self.sender_proto_addr)
    }
    #[inline]
    pub fn target_proto_addr(&self) -> u32 {
        u32::from_be(self.target_proto_addr)
    }
    #[inline]
    pub fn target_ip_addr(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.target_proto_addr())
    }
    #[inline]
    pub fn sender_ip_addr(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.sender_proto_addr())
    }
}
