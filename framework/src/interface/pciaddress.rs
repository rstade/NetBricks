use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum PciError {
    InvalidFormat(String),
    IoError(io::Error),
    NotFound,
}

impl std::fmt::Display for PciError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PciError::InvalidFormat(msg) => write!(f, "illegal PCI format: {}", msg),
            PciError::IoError(e) => write!(f, "I/O-error: {}", e),
            PciError::NotFound => write!(f, "No interface found for this PCI address"),
        }
    }
}

impl std::error::Error for PciError {}

impl From<io::Error> for PciError {
    fn from(err: io::Error) -> Self {
        PciError::IoError(err)
    }
}

/// Parsed PCI address
#[derive(Debug, Clone)]
pub struct PciAddress {
    pub bus: u8,
    pub slot: u8,
    pub function: u8,
}

impl PciAddress {
    /// Parses a PCI address in the format "Bus:Slot.Function" (e.g. "03:00.0")
    pub fn parse(addr: &str) -> Result<Self, PciError> {
        let parts: Vec<&str> = addr.split(':').collect();
        if parts.len() != 2 {
            return Err(PciError::InvalidFormat(
                "Expected format 'Bus:Slot.Function' (e.g. '03:00.0')".to_string(),
            ));
        }

        let bus = u8::from_str_radix(parts[0], 16)
            .map_err(|_| PciError::InvalidFormat("Bus must be hexadecimal".to_string()))?;

        let slot_func: Vec<&str> = parts[1].split('.').collect();
        if slot_func.len() != 2 {
            return Err(PciError::InvalidFormat("Expected format 'Slot.Function'".to_string()));
        }

        let slot = u8::from_str_radix(slot_func[0], 16)
            .map_err(|_| PciError::InvalidFormat("Slot must be hexadecimal".to_string()))?;

        let function = u8::from_str_radix(slot_func[1], 16)
            .map_err(|_| PciError::InvalidFormat("Function must be hexadecimal".to_string()))?;

        Ok(PciAddress { bus, slot, function })
    }

    /// Converts to PCI address in the format "0000:BB:SS.F"
    pub fn to_full_address(&self) -> String {
        format!("0000:{:02x}:{:02x}.{}", self.bus, self.slot, self.function)
    }
}

/// Finds the Ethernet interface name for a given PCI address
pub fn pci_to_interface(pci_addr: &str) -> Result<String, PciError> {
    let addr = PciAddress::parse(pci_addr)?;
    let full_pci_addr = addr.to_full_address();

    // Search /sys/class/net/ for interfaces
    let net_path = PathBuf::from("/sys/class/net");

    if !net_path.exists() {
        return Err(PciError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            "/sys/class/net not found (is this running on Linux?)",
        )));
    }

    let entries = fs::read_dir(&net_path)?;

    for entry in entries {
        let entry = entry?;
        let interface_name = entry.file_name();
        let interface_name_str = interface_name.to_string_lossy();

        // Skip loopback and other virtual interfaces
        if interface_name_str == "lo" {
            continue;
        }

        // Check if the interface has a PCI device
        let device_path = entry.path().join("device");
        if !device_path.exists() {
            continue;
        }

        // Read the symbolic link to obtain the PCI address
        if let Ok(link_target) = fs::read_link(&device_path) {
            if let Some(target_str) = link_target.to_str() {
                // The link points to something like "../../devices/pci0000:00/0000:00:1f.6"
                if target_str.contains(&full_pci_addr) {
                    return Ok(interface_name_str.to_string());
                }
            }
        }
    }

    Err(PciError::NotFound)
}
