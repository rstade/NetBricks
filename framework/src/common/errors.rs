use ipnet;
use std::fmt::{Display, Formatter};
use std::net::AddrParseError;
use std::string::String;

/*
error_chain! {
    errors {
        FailedAllocation {
            description("Failed to allocate memory")
            display("Failed to allocate memory")
        }
        FailedDeallocation {
            description("Failed to deallocate memory")
            display("Failed to deallocate memory")
        }
        FailedToInitializePort(port_id: u16) {
            description("Failed to initialize port")
            display("Failed to initialize port: {}", port_id)
        }
        FailedToInitializeOvsPort(ret_code: i32) {
            description("Failed to initialize ovs port")
            display("Failed to initialize ovs port, error code: {}", ret_code)
        }
        FailedToInitializeBessPort(ret_code: i32) {
            description("Failed to initialize bess port")
            display("Failed to initialize bess port, error code: {}", ret_code)
        }
        FailedToInitializeKni(port_id: u16) {
            description("Failed to initialize kni i/f")
            display("Failed to initialize kni i/f: port_id={}", port_id)
        }
        BadQueue {
            description("Invalid queue request")
            display("Invalid queue request")
        }
        CannotSend {
            description("Cannot send data out port")
            display("Cannot send data out port")
        }
        BadDev(dev: String) {
            description("Cannot find device")
            display("Cannot find device: {}", dev)
        }
        BadVdev(vdev: String) {
            description("Bad vdev specification")
            display("Bad vdev specification: {}", vdev)
        }
        BadTxQueue(port_id: u16, queue: u16) {
            description("Bad TX queue")
            display("Bad TX queue {} for port {}", queue, port_id)
        }
        BadRxQueue(port_id: u16, queue: u16) {
            description("Bad RX queue")
            display("Bad RX queue {} for port {}", queue, port_id)
        }
        BadOffset(offset: usize) {
            description("Attempt to access bad packet offset")
            display("Attempt to access bad packet offset {}", offset)
        }

        MetadataTooLarge {
            description("Metadata is too large")
            display("Metadata is too large")
        }

        RingAllocationFailure {
            description("Could not allocate ring")
            display("Could not allocate ring")
        }

        InvalidRingSize(size: usize) {
            description("Bad ring size, must be power of 2")
            display("Bad ring size {}, must be a power of 2", size)
        }

        RingDuplicationFailure {
            description("Address of second copy of ring does not match expected address")
            display("Address of second copy of ring does not match expected address")
        }

        ConfigurationError(description: String) {
            description("Configuration error")
            display("Configuration error: {}", description)
        }

        NoRunningSchedulerOnCore(core: i32) {
            description("No scheduler running on core")
            display("No scheduler running on core {}", core)
        }

        BadSize(sz: usize, description: String) {
            description("Bad size")
            display("Bad size {}, {}", sz, description)
        }

        BadCharAtIndex(c: char, index: usize) {
            description("Bad char")
            display("Bad char {} at index {}", c, index)
        }

        HeaderMismatch {
            description("Wrong header")
            display("Wrong header")
        }
    }

    foreign_links {
        Io(::std::io::Error);
        AddrParse(::std::net::AddrParseError);
        Toml(::toml::de::Error);
    }
}
*/

#[derive(Debug)]
pub enum ErrorKind {
    FailedAllocation,
    FailedDeallocation,
    FailedToInitializePort(u16),
    FailedToInitializeOvsPort(i32),
    FailedToInitializeBessPort(i32),
    FailedToInitializeKni(String),
    BadQueue,
    CannotSend,
    BadDev(String),
    BadVdev(String),
    BadTxQueue(u16, u16),
    BadRxQueue(u16, u16),
    BadOffset(usize),
    MetadataTooLarge,
    RingAllocationFailure,
    InvalidRingSize(usize),
    RingDuplicationFailure,
    ConfigurationError(String),
    RunTimeError(String),
    NoRunningSchedulerOnCore(i32),
    BadSize(usize, String),
    BadCharAtIndex(char, usize),
    HeaderMismatch,
    FailedErrorFormat,
    ConfigParseError(String),
    TryFromNetSpecError,
}

impl From<AddrParseError> for ErrorKind {
    fn from(err: AddrParseError) -> Self {
        ErrorKind::ConfigParseError(format!("{}", err))
    }
}

impl From<toml::de::Error> for ErrorKind {
    fn from(err: toml::de::Error) -> Self {
        ErrorKind::ConfigParseError(format!("{}", err))
    }
}

impl From<eui48::ParseError> for ErrorKind {
    fn from(err: eui48::ParseError) -> Self {
        ErrorKind::ConfigParseError(format!("{}", err))
    }
}

impl From<ipnet::AddrParseError> for ErrorKind {
    fn from(err: ipnet::AddrParseError) -> Self {
        ErrorKind::ConfigParseError(format!("{}", err))
    }
}

//TODO improve Display

impl Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

pub type Result<T> = std::result::Result<T, ErrorKind>;
