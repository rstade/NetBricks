use std::fmt;

pub use self::ip::*;
pub use self::mac::*;
pub use self::null_header::*;
pub use self::tcp::*;
pub use self::udp::*;

mod ip;
mod mac;
mod null_header;
mod tcp;
mod udp;

#[derive(Debug, PartialEq)]
pub enum HeaderKind {
    Null,
    Mac,
    Ip,
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

#[derive(Debug)]
pub enum Header<'a> {
    Null,
    Mac(&'a mut MacHeader),
    Ip(&'a mut IpHeader),
    Tcp(&'a mut TcpHeader),
    Udp(&'a mut UdpHeader),
}


///as Header contains mutable references, we can only clone Header::Null
///we need this for initialization of arrays
impl<'a> Clone for Header<'a>{
    fn clone(&self) -> Self {
        Header::Null
    }
}

impl<'a> Header<'a> {

    pub fn new<T:EndOffset>(ptr: *mut T) -> Header<'a> {
        unsafe { match (*ptr).header_kind()  {
            HeaderKind::Null => Header::Null,
            HeaderKind::Mac  => Header::Mac(& mut *(ptr as *mut MacHeader)),
            HeaderKind::Ip   => Header::Ip(& mut *(ptr as *mut IpHeader)),
            HeaderKind::Tcp  => Header::Tcp(& mut *(ptr as *mut TcpHeader)),
            HeaderKind::Udp  => Header::Udp(& mut *(ptr as *mut UdpHeader)),
        } }
    }

    #[inline]
    pub fn as_mac_mut(&mut self) -> Option<&mut MacHeader> {
        match self {
            Header::Mac(p) => Some( &mut **p ),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ip_mut(&mut self) -> Option<&mut IpHeader> {
        match self {
            Header::Ip(p) => Some( &mut **p ),
            _ => None,
        }
    }

    #[inline]
    pub fn as_tcp_mut(&mut self) -> Option<&mut TcpHeader> {
        match self {
            Header::Tcp(p) => Some( &mut **p ),
            _ => None,
        }
    }

    #[inline]
    pub fn as_udp_mut(&mut self) -> Option<&mut UdpHeader> {
        match self {
            Header::Udp(p) => Some( &mut **p ),
            _ => None,
        }
    }

    #[inline]
    pub fn as_mac(&self) -> Option<&MacHeader> {
        match self {
            Header::Mac(p) => Some( & **p ),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ip(&self) -> Option<&IpHeader> {
        match self {
            Header::Ip(p) => Some( & **p ),
            _ => None,
        }
    }

    #[inline]
    pub fn as_tcp(&self) -> Option<&TcpHeader> {
        match self {
            Header::Tcp(p) => Some( & **p ),
            _ => None,
        }
    }

    #[inline]
    pub fn as_udp(&self) -> Option<&UdpHeader> {
        match self {
            Header::Udp(p) => Some( & **p ),
            _ => None,
        }
    }

    #[inline]
    pub fn kind(&self) -> HeaderKind {
        match self {
            Header::Null => HeaderKind::Null,
            Header::Mac(_) => HeaderKind::Mac,
            Header::Ip(_) => HeaderKind::Ip,
            Header::Tcp(_) => HeaderKind::Tcp,
            Header::Udp(_) => HeaderKind::Udp,
        }
    }

    #[inline]
    pub fn offset(&self) -> Option<usize> {
        match self {
            Header::Null => None,
            Header::Mac(_) => Some(self.as_mac().unwrap().offset()),
            Header::Ip(_) => Some(self.as_ip().unwrap().offset()),
            Header::Tcp(_) => Some(self.as_tcp().unwrap().offset()),
            Header::Udp(_) => Some(self.as_udp().unwrap().offset()),
        }
    }

    #[inline]
    pub fn as_ptr_u8_mut(&mut self) -> Option<*mut u8> {
        match self {
            Header::Null => None,
            Header::Mac(p) => Some(*p as *mut MacHeader as *mut u8),
            Header::Ip(p) => Some(*p as *mut IpHeader as *mut u8),
            Header::Tcp(p) => Some(*p as *mut TcpHeader as *mut u8),
            Header::Udp(p) => Some(*p as *mut UdpHeader as *mut u8),
        }
    }

    #[inline]
    pub fn as_ptr_u8(&self) -> Option<*const u8> {
        match self {
            Header::Null => None,
            Header::Mac(p) => Some(*p as *const MacHeader as *const u8),
            Header::Ip(p) => Some(*p as *const IpHeader as *const u8),
            Header::Tcp(p) => Some(*p as *const TcpHeader as *const u8),
            Header::Udp(p) => Some(*p as *const UdpHeader as *const u8),
        }
    }
}

impl<'a> fmt::Display for Header<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            Header::Null => write!(f, "{:?}", self),
            Header::Mac(_) => write!(f, "{:?}", self.as_mac().unwrap()),
            Header::Ip(_) => write!(f, "{ }", self.as_ip().unwrap()),
            Header::Tcp(_) => write!(f, "{ }", self.as_tcp().unwrap()),
            Header::Udp(_) => write!(f, "{:?}", self.as_udp().unwrap()),
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
