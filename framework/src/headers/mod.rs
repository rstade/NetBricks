use std::fmt;

pub use self::arp::*;
pub use self::ip::*;
pub use self::ipv6::*;
pub use self::mac::*;
pub use self::null_header::*;
pub use self::tcp::*;
pub use self::udp::*;

mod arp;
mod ip;
mod ipv6;
mod mac;
mod null_header;
mod tcp;
mod udp;

#[derive(Debug, PartialEq)]
pub enum HeaderKind {
    Null,
    Mac,
    ArpIpv4,
    Ip,
    Ipv6,
    Tcp,
    Udp,
}

/// A trait implemented by all headers, used for reading them from a mbuf.
pub trait EndOffset: Send {
    /// Offset returns the number of bytes to skip to get to the next header, relative to the start
    /// of the mbuf.
    fn offset(&self) -> usize;

    /// Returns the size of this header in bytes.
    fn size() -> usize;

    /// Returns the size of the payload in bytes. The hint is necessary for things like the L2 header which have no
    /// explicit length field.
    fn payload_size(&self, hint: usize) -> usize;

    fn header_kind(&self) -> HeaderKind;
}

//Replace the current, lifetime-bearing enum with a pointer-backed version.
// New internal representation (no lifetime parameter)
#[derive(Debug)]
pub enum HeaderPtr {
    Null,
    Mac(*mut MacHeader),
    ArpIpv4(*mut ArpIpv4Header),
    Ip(*mut IpHeader),
    Ipv6(*mut Ipv6Header),
    Tcp(*mut TcpHeader),
    Udp(*mut UdpHeader),
}

// Keep initialization ergonomics
impl HeaderPtr {
    #[inline]
    pub fn new<T: EndOffset>(ptr: *mut T) -> HeaderPtr {
        unsafe {
            match (*ptr).header_kind() {
                HeaderKind::Null => HeaderPtr::Null,
                HeaderKind::Mac => HeaderPtr::Mac(ptr as *mut MacHeader),
                HeaderKind::Ip => HeaderPtr::Ip(ptr as *mut IpHeader),
                HeaderKind::Ipv6 => HeaderPtr::Ipv6(ptr as *mut Ipv6Header),
                HeaderKind::Tcp => HeaderPtr::Tcp(ptr as *mut TcpHeader),
                HeaderKind::Udp => HeaderPtr::Udp(ptr as *mut UdpHeader),
                HeaderKind::ArpIpv4 => HeaderPtr::ArpIpv4(ptr as *mut ArpIpv4Header),
            }
        }
    }

    // Accessors create temporary borrows on demand (no persistent &mut stored)
    #[inline]
    pub fn as_mac_mut(&mut self) -> Option<&mut MacHeader> {
        match self {
            HeaderPtr::Mac(p) => Some(unsafe { &mut **p }),
            _ => None,
        }
    }
    #[inline]
    pub fn as_mac(&self) -> Option<&MacHeader> {
        match self {
            HeaderPtr::Mac(p) => Some(unsafe { &**p }),
            _ => None,
        }
    }
    // ArpIpv4 accessors
    #[inline]
    pub fn as_arpipv4_mut(&mut self) -> Option<&mut ArpIpv4Header> {
        match self {
            HeaderPtr::ArpIpv4(p) => Some(unsafe { &mut **p }),
            _ => None,
        }
    }
    #[inline]
    pub fn as_arpipv4(&self) -> Option<&ArpIpv4Header> {
        match self {
            HeaderPtr::ArpIpv4(p) => Some(unsafe { &**p }),
            _ => None,
        }
    }

    // Ip accessors
    #[inline]
    pub fn as_ip_mut(&mut self) -> Option<&mut IpHeader> {
        match self {
            HeaderPtr::Ip(p) => Some(unsafe { &mut **p }),
            _ => None,
        }
    }
    #[inline]
    pub fn as_ip(&self) -> Option<&IpHeader> {
        match self {
            HeaderPtr::Ip(p) => Some(unsafe { &**p }),
            _ => None,
        }
    }

    // Ipv6 accessors
    #[inline]
    pub fn as_ipv6_mut(&mut self) -> Option<&mut Ipv6Header> {
        match self {
            HeaderPtr::Ipv6(p) => Some(unsafe { &mut **p }),
            _ => None,
        }
    }
    #[inline]
    pub fn as_ipv6(&self) -> Option<&Ipv6Header> {
        match self {
            HeaderPtr::Ipv6(p) => Some(unsafe { &**p }),
            _ => None,
        }
    }

    // Tcp accessors
    #[inline]
    pub fn as_tcp_mut(&mut self) -> Option<&mut TcpHeader> {
        match self {
            HeaderPtr::Tcp(p) => Some(unsafe { &mut **p }),
            _ => None,
        }
    }
    #[inline]
    pub fn as_tcp(&self) -> Option<&TcpHeader> {
        match self {
            HeaderPtr::Tcp(p) => Some(unsafe { &**p }),
            _ => None,
        }
    }

    // Udp accessors
    #[inline]
    pub fn as_udp_mut(&mut self) -> Option<&mut UdpHeader> {
        match self {
            HeaderPtr::Udp(p) => Some(unsafe { &mut **p }),
            _ => None,
        }
    }
    #[inline]
    pub fn as_udp(&self) -> Option<&UdpHeader> {
        match self {
            HeaderPtr::Udp(p) => Some(unsafe { &**p }),
            _ => None,
        }
    }

    #[inline]
    pub fn kind(&self) -> HeaderKind {
        match self {
            HeaderPtr::Null => HeaderKind::Null,
            HeaderPtr::Mac(_) => HeaderKind::Mac,
            HeaderPtr::ArpIpv4(_) => HeaderKind::ArpIpv4,
            HeaderPtr::Ip(_) => HeaderKind::Ip,
            HeaderPtr::Ipv6(_) => HeaderKind::Ipv6,
            HeaderPtr::Tcp(_) => HeaderKind::Tcp,
            HeaderPtr::Udp(_) => HeaderKind::Udp,
        }
    }

    // Raw pointer views used by payload computations
    #[inline]
    pub fn as_ptr_u8(&self) -> Option<*const u8> {
        match self {
            HeaderPtr::Null => None,
            HeaderPtr::Mac(p) => Some(*p as *const u8),
            HeaderPtr::ArpIpv4(p) => Some(*p as *const u8),
            HeaderPtr::Ip(p) => Some(*p as *const u8),
            HeaderPtr::Ipv6(p) => Some(*p as *const u8),
            HeaderPtr::Tcp(p) => Some(*p as *const u8),
            HeaderPtr::Udp(p) => Some(*p as *const u8),
        }
    }
    #[inline]
    pub fn as_ptr_u8_mut(&mut self) -> Option<*mut u8> {
        match self {
            HeaderPtr::Null => None,
            HeaderPtr::Mac(p) => Some(*p as *mut u8),
            HeaderPtr::ArpIpv4(p) => Some(*p as *mut u8),
            HeaderPtr::Ip(p) => Some(*p as *mut u8),
            HeaderPtr::Ipv6(p) => Some(*p as *mut u8),
            HeaderPtr::Tcp(p) => Some(*p as *mut u8),
            HeaderPtr::Udp(p) => Some(*p as *mut u8),
        }
    }
}

// Preserve existing initialization behavior for arrays
impl Clone for HeaderPtr {
    fn clone(&self) -> Self {
        HeaderPtr::Null
    }
}

// Keep the public name and lifetime parameter to avoid signature churn
pub type Header = HeaderPtr; // 'a is unused but preserves existing signatures

impl HeaderPtr {
    #[inline]
    pub fn offset(&self) -> Option<usize> {
        match self {
            Header::Null => None,
            Header::Mac(_) => Some(self.as_mac().unwrap().offset()),
            Header::Ip(_) => Some(self.as_ip().unwrap().offset()),
            Header::Ipv6(_) => Some(self.as_ipv6().unwrap().offset()),
            Header::Tcp(_) => Some(self.as_tcp().unwrap().offset()),
            Header::Udp(_) => Some(self.as_udp().unwrap().offset()),
            Header::ArpIpv4(_) => Some(self.as_arpipv4().unwrap().offset()),
        }
    }
}

impl<'a> fmt::Display for HeaderPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Header::Null => write!(f, "{:?}", self),
            Header::Mac(_) => write!(f, "{:?}", self.as_mac().unwrap()),
            Header::Ip(_) => write!(f, "{ }", self.as_ip().unwrap()),
            Header::Ipv6(_) => write!(f, "{ }", self.as_ipv6().unwrap()),
            Header::Tcp(_) => write!(f, "{ }", self.as_tcp().unwrap()),
            Header::Udp(_) => write!(f, "{:?}", self.as_udp().unwrap()),
            Header::ArpIpv4(_) => write!(f, "{:?}", self.as_arpipv4().unwrap()),
        }
    }
}

#[test]

fn test_headers() {
    let mut ip_header = IpHeader::new();
    println!("ip_header= {:?}", ip_header);
    let header = Header::Ip(&mut ip_header);
    println!("header= {}, header.kind= {:?}", header, header.kind());
    assert_eq!(header.kind(), HeaderKind::Ip);
    assert!(header.as_ip().is_some());
    assert!(header.as_mac().is_none());
    assert!(header.as_tcp().is_none());
    assert!(header.as_udp().is_none());
}
