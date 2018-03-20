use super::{EndOffset, Header};
use headers::NullHeader;
use std::default::Default;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::cmp::Ordering;
use std;

// merged in lot of https://github.com/abaumhauer/eui48/blob/master/src/lib.rs

/// A 48-bit (6 byte) buffer containing the EUI address
pub const EUI48LEN: usize = 6;
pub type Eui48 = [u8; EUI48LEN];

#[derive(Debug)]
pub enum ParseError {
    /// Length is incorrect (should be 14 or 17)
    InvalidLength(usize),
    /// Character not [0-9a-fA-F]|'x'|'-'|':'|'.'
    InvalidCharacter(char, usize),
    IOError(std::io::Error),
}



#[derive(Debug, Default, Copy)]
#[repr(C, packed)]
pub struct MacAddress {
    pub addr: Eui48,
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.addr[0],
            self.addr[1],
            self.addr[2],
            self.addr[3],
            self.addr[4],
            self.addr[5]
        )
    }
}

impl MacAddress {
    pub fn new(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8) -> MacAddress {
        MacAddress { addr: [a, b, c, d, e, f] }
    }

    pub fn new_from_eui48(eui: Eui48) -> MacAddress {
        MacAddress { addr: eui }
    }


    pub fn new_from_slice(slice: &[u8]) -> MacAddress {
        MacAddress { addr: [slice[0], slice[1], slice[2], slice[3], slice[4], slice[5]] }
    }

    #[inline]
    pub fn copy_address(&mut self, other: &MacAddress) {
        self.addr.copy_from_slice(&other.addr);
    }

    /// Returns empty EUI-48 address
    pub fn nil() -> MacAddress {
        MacAddress { addr: [0; 6] }
    }

    /// Returns 'ff:ff:ff:ff:ff:ff', a MAC broadcast address
    pub fn broadcast() -> MacAddress {
        MacAddress { addr: [0xFF; 6] }
    }

    /// Returns true if the address is '00:00:00:00:00:00'
    pub fn is_nil(&self) -> bool {
        self.addr.iter().all(|&b| b == 0)
    }

    /// Returns true if the address is 'ff:ff:ff:ff:ff:ff'
    pub fn is_broadcast(&self) -> bool {
        self.addr.iter().all(|&b| b == 0xFF)
    }

    /// Returns true if bit 1 of Y is 1 in address 'xY:xx:xx:xx:xx:xx'
    pub fn is_unicast(&self) -> bool {
        self.addr[0] & 0 == 0
    }

    /// Returns true if bit 1 of Y is 1 in address 'xY:xx:xx:xx:xx:xx'
    pub fn is_multicast(&self) -> bool {
        self.addr[0] & 1 != 0
    }

    /// Returns true if bit 2 of Y is 0 in address 'xY:xx:xx:xx:xx:xx'
    pub fn is_universal(&self) -> bool {
        self.addr[0] & 1 << 1 == 0
    }

    /// Returns true if bit 2 of Y is 1 in address 'xY:xx:xx:xx:xx:xx'
    pub fn is_local(&self) -> bool {
        self.addr[0] & 1 << 1 != 0
    }

    /// Returns a String representation in the format '00-00-00-00-00-00'
    pub fn to_canonical(&self) -> String {
        format!(
            "{:02x}-{:02x}-{:02x}-{:02x}-{:02x}-{:02x}",
            self.addr[0],
            self.addr[1],
            self.addr[2],
            self.addr[3],
            self.addr[4],
            self.addr[5]
        )
    }

    /// Returns a String representation in the format '00:00:00:00:00:00'
    pub fn to_hex_string(&self) -> String {
        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.addr[0],
            self.addr[1],
            self.addr[2],
            self.addr[3],
            self.addr[4],
            self.addr[5]
        )
    }

    /// Returns a String representation in the format '0000.0000.0000'
    pub fn to_dot_string(&self) -> String {
        format!(
            "{:02x}{:02x}.{:02x}{:02x}.{:02x}{:02x}",
            self.addr[0],
            self.addr[1],
            self.addr[2],
            self.addr[3],
            self.addr[4],
            self.addr[5]
        )
    }

    /// Returns a String representation in the format '0x000000000000'
    pub fn to_hexadecimal(&self) -> String {
        format!(
            "0x{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.addr[0],
            self.addr[1],
            self.addr[2],
            self.addr[3],
            self.addr[4],
            self.addr[5]
        )
    }

    /// Parses a String representation from any format supported
    pub fn parse_str(s: &str) -> Result<MacAddress, ParseError> {
        let mut offset = 0; // Offset into the u8 Eui48 vector
        let mut hn: bool = false; // Have we seen the high nibble yet?
        let mut eui: Eui48 = [0; EUI48LEN];

        match s.len() {
            14 | 17 => {}  // The formats are all 12 characters with 2 or 5 delims
            _ => return Err(ParseError::InvalidLength(s.len())),
        }

        for (idx, c) in s.chars().enumerate() {
            if offset >= EUI48LEN {
                // We shouln't still be parsing
                return Err(ParseError::InvalidLength(s.len()));
            }

            match c {
                '0'...'9' | 'a'...'f' | 'A'...'F' => {
                    match hn {
                        false => {
                            // We will match '0' and run this even if the format is 0x
                            hn = true; // Parsed the high nibble
                            eui[offset] = (c.to_digit(16).unwrap() as u8) << 4;
                        }
                        true => {
                            hn = false; // Parsed the low nibble
                            eui[offset] += c.to_digit(16).unwrap() as u8;
                            offset += 1;
                        }
                    }
                }
                '-' | ':' | '.' => {}
                'x' | 'X' => {
                    match idx {
                        1 => {
                            // If idx = 1, we are possibly parsing 0x1234567890ab format
                            // Reset the offset to zero to ignore the first two characters
                            offset = 0;
                            hn = false;
                        }
                        _ => return Err(ParseError::InvalidCharacter(c, idx)),
                    }
                }
                _ => return Err(ParseError::InvalidCharacter(c, idx)),
            }
        }

        if offset == EUI48LEN {
            // A correctly parsed value is exactly 6 u8s
            Ok(MacAddress::new_from_eui48(eui))
        } else {
            Err(ParseError::InvalidLength(s.len())) // Something slipped through
        }
    }
}

impl Clone for MacAddress {
    fn clone(&self) -> MacAddress {
        let mut m: MacAddress = Default::default();
        m.addr.copy_from_slice(&self.addr);
        m
    }
    fn clone_from(&mut self, source: &MacAddress) {
        self.addr.copy_from_slice(&source.addr)
    }
}

impl PartialEq for MacAddress {
    fn eq(&self, other: &MacAddress) -> bool {
        self.addr == other.addr
    }
}

impl Eq for MacAddress {}

impl Ord for MacAddress {
    fn cmp(&self, other: &MacAddress) -> Ordering {
        self.addr.cmp(&other.addr)
    }
}

impl PartialOrd for MacAddress {
    fn partial_cmp(&self, other: &MacAddress) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


impl Hash for MacAddress {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.addr.hash(state);
    }
}

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
        let mut src: MacAddress = Default::default();
        src.copy_address(&self.src);
        self.src.copy_address(&self.dst);
        self.dst.copy_address(&src);
    }

    pub fn set_dmac(&mut self, dmac: &MacAddress) {
        self.dst.copy_address(dmac);
    }

    pub fn set_smac(&mut self, smac: &MacAddress) {
        self.src.copy_address(smac);
    }
}
