use super::{EndOffset, HeaderKind};
use std::convert::From;
use std::default::Default;
use std::fmt;
use std::net::Ipv6Addr;

/// IPv6 header structure (40 bytes fixed size)
#[derive(Clone, Copy, Debug, Default)]
#[repr(C, packed)]
pub struct Ipv6Header {
    /// Version (4 bits), Traffic Class (8 bits), Flow Label (20 bits)
    version_to_flow: u32,
    /// Payload Length (16 bits)
    payload_len: u16,
    /// Next Header (8 bits) - Protocol type
    next_header: u8,
    /// Hop Limit (8 bits) - Similar to TTL in IPv4
    hop_limit: u8,
    /// Source IPv6 address (128 bits / 16 bytes)
    src_ip: [u8; 16],
    /// Destination IPv6 address (128 bits / 16 bytes)
    dst_ip: [u8; 16],
}

impl fmt::Display for Ipv6Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let src = self.src();
        let dst = self.dst();
        write!(
            f,
            "{} > {} version: {} traffic_class: {} flow_label: {} payload_len: {} next_header: {} hop_limit: {}",
            src,
            dst,
            self.version(),
            self.traffic_class(),
            self.flow_label(),
            self.payload_length(),
            self.next_header(),
            self.hop_limit()
        )
    }
}

impl EndOffset for Ipv6Header {
    #[inline]
    fn offset(&self) -> usize {
        // IPv6 header is always 40 bytes (no variable length like IPv4)
        40
    }

    #[inline]
    fn size() -> usize {
        40
    }

    #[inline]
    fn payload_size(&self, _: usize) -> usize {
        self.payload_length() as usize
    }

    #[inline]
    fn header_kind(&self) -> HeaderKind {
        HeaderKind::Ipv6
    }
}

impl Ipv6Header {
    #[inline]
    pub fn new() -> Ipv6Header {
        let mut header = Ipv6Header::default();
        header.set_version(6);
        header
    }

    /// Get version (should always be 6 for IPv6)
    #[inline]
    pub fn version(&self) -> u8 {
        (u32::from_be(self.version_to_flow) >> 28) as u8
    }

    /// Set version (should be 6)
    #[inline]
    pub fn set_version(&mut self, version: u8) {
        let vtf = u32::from_be(self.version_to_flow);
        self.version_to_flow = u32::to_be((vtf & 0x0FFFFFFF) | ((version as u32 & 0xF) << 28));
    }

    /// Get traffic class (similar to DSCP/ToS in IPv4)
    #[inline]
    pub fn traffic_class(&self) -> u8 {
        ((u32::from_be(self.version_to_flow) >> 20) & 0xFF) as u8
    }

    /// Set traffic class
    #[inline]
    pub fn set_traffic_class(&mut self, tc: u8) {
        let vtf = u32::from_be(self.version_to_flow);
        self.version_to_flow = u32::to_be((vtf & 0xF00FFFFF) | ((tc as u32) << 20));
    }

    /// Get flow label (20 bits)
    #[inline]
    pub fn flow_label(&self) -> u32 {
        u32::from_be(self.version_to_flow) & 0x000FFFFF
    }

    /// Set flow label
    #[inline]
    pub fn set_flow_label(&mut self, label: u32) {
        let vtf = u32::from_be(self.version_to_flow);
        self.version_to_flow = u32::to_be((vtf & 0xFFF00000) | (label & 0x000FFFFF));
    }

    /// Get payload length (in bytes, not including IPv6 header)
    #[inline]
    pub fn payload_length(&self) -> u16 {
        u16::from_be(self.payload_len)
    }

    /// Set payload length
    #[inline]
    pub fn set_payload_length(&mut self, len: u16) {
        self.payload_len = u16::to_be(len);
    }

    /// Get next header (protocol type)
    #[inline]
    pub fn next_header(&self) -> u8 {
        self.next_header
    }

    /// Set next header
    #[inline]
    pub fn set_next_header(&mut self, nh: u8) {
        self.next_header = nh;
    }

    /// Get hop limit (similar to TTL in IPv4)
    #[inline]
    pub fn hop_limit(&self) -> u8 {
        self.hop_limit
    }

    /// Set hop limit
    #[inline]
    pub fn set_hop_limit(&mut self, hl: u8) {
        self.hop_limit = hl;
    }

    /// Get source IPv6 address
    #[inline]
    pub fn src(&self) -> Ipv6Addr {
        Ipv6Addr::from(self.src_ip)
    }

    /// Set source IPv6 address
    #[inline]
    pub fn set_src(&mut self, addr: Ipv6Addr) {
        self.src_ip = addr.octets();
    }

    /// Get destination IPv6 address
    #[inline]
    pub fn dst(&self) -> Ipv6Addr {
        Ipv6Addr::from(self.dst_ip)
    }

    /// Set destination IPv6 address
    #[inline]
    pub fn set_dst(&mut self, addr: Ipv6Addr) {
        self.dst_ip = addr.octets();
    }

    /// Get source IPv6 address as raw bytes
    #[inline]
    pub fn src_bytes(&self) -> &[u8; 16] {
        &self.src_ip
    }

    /// Get destination IPv6 address as raw bytes
    #[inline]
    pub fn dst_bytes(&self) -> &[u8; 16] {
        &self.dst_ip
    }
}
