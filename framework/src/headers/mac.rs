use super::{EndOffset, Header};
use headers::NullHeader;
use std::default::Default;
use std::fmt;
use eui48::{MacAddress};



/// A packet's MAC header.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C, packed)]
pub struct MacHeader {
    pub dst: MacAddress,
    pub src: MacAddress,
    etype: u16,
}

impl fmt::Display for MacHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} > {} 0x{:04x}",
            self.src,
            self.dst,
            u16::from_be(self.etype)
        )
    }
}

const HDR_SIZE: usize = 14;
const HDR_SIZE_802_1Q: usize = HDR_SIZE + 4;
const HDR_SIZE_802_1AD: usize = HDR_SIZE_802_1Q + 4;

impl EndOffset for MacHeader {
    type PreviousHeader = NullHeader;
    #[inline]
    fn offset(&self) -> usize {
        if cfg!(feature = "performance") {
            HDR_SIZE
        } else {
            match self.etype {
                0x8100 => HDR_SIZE_802_1Q,
                0x9100 => HDR_SIZE_802_1AD,
                _ => HDR_SIZE,
            }
        }
    }
    #[inline]
    fn size() -> usize {
        // The struct itself is always 20 bytes. Really ?????
        HDR_SIZE
    }

    #[inline]
    fn payload_size(&self, hint: usize) -> usize {
        hint - self.offset()
    }

    #[inline]
    fn check_correct(&self, _: &NullHeader) -> bool {
        true
    }

    #[inline]
    fn is_header(&self) -> Header {
        Header::Mac
    }
}

impl MacHeader {
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    pub fn etype(&self) -> u16 {
        u16::from_be(self.etype)
    }

    #[inline]
    pub fn set_etype(&mut self, etype: u16) {
        self.etype = u16::to_be(etype)
    }

    #[inline]
    pub fn swap_addresses(&mut self) {
        let src: MacAddress = self.src;
        self.src= self.dst;
        self.dst= src;
    }

    pub fn set_dmac(&mut self, dmac: &MacAddress) {
        self.dst= *dmac;
    }

    pub fn set_smac(&mut self, smac: &MacAddress) {
        self.src= *smac;
    }
}
