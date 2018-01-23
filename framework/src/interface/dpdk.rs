use super::METADATA_SLOTS;
use config::{DEFAULT_CACHE_SIZE, DEFAULT_POOL_SIZE, NetbricksConfiguration};
use native::libnuma;
use native::zcsi;
use std::cell::Cell;
use std::ffi::CString;

/// Initialize the system, whitelisting some set of NICs and allocating mempool of given size.
fn init_system_wl_with_mempool(
    name: &str,
    lcore_mask: u64,
    core: i32,
    pci: &[String],
    pool_size: u32,
    cache_size: u32,
    vdevs: &Vec<String>,
) {
    let name_cstr = CString::new(name).unwrap();
    let pci_cstr: Vec<_> = pci.iter().map(|p| CString::new(&p[..]).unwrap()).collect();
    let mut whitelist: Vec<_> = pci_cstr.iter().map(|p| p.as_ptr()).collect();
    let vdevs_cstr: Vec<_> = vdevs
        .iter()
        .map(|p| CString::new(&p[..]).unwrap())
        .collect();
    let mut vdevs_ptr: Vec<_> = vdevs_cstr.iter().map(|p| p.as_ptr()).collect();
    unsafe {
        let ret = zcsi::init_system_whitelisted(
            name_cstr.as_ptr(),
            name.len() as i32,
            lcore_mask,
            core,
            whitelist.as_mut_ptr(),
            pci.len() as i32,
            pool_size,
            cache_size,
            METADATA_SLOTS,
            vdevs_ptr.as_mut_ptr(),
            vdevs.len() as i32,
        );
        if ret != 0 {
            panic!("Could not initialize the system errno {}", ret)
        }
    }
}

/// Initialize the system, whitelisting some set of NICs.
pub fn init_system_wl(name: &str, lcore_mask: u64, core: i32, pci: &[String], vdevs: &Vec<String>) {
    init_system_wl_with_mempool(
        name,
        lcore_mask,
        core,
        pci,
        DEFAULT_POOL_SIZE,
        DEFAULT_CACHE_SIZE,
        vdevs,
    );
    set_numa_domain();
}

/// Initialize the system as a DPDK secondary process with a set of VDEVs. User must specify mempool name to use.
pub fn init_system_secondary(name: &str, lcore_mask: u64, core: i32) {
    let name_cstr = CString::new(name).unwrap();
    let mut vdev_list = vec![];
    unsafe {
        let ret = zcsi::init_secondary(
            name_cstr.as_ptr(),
            name.len() as i32,
            lcore_mask,
            core,
            vdev_list.as_mut_ptr(),
            0,
        );
        if ret != 0 {
            panic!("Could not initialize secondary process errno {}", ret)
        }
    }
    set_numa_domain();
}

/// Initialize the system based on the supplied scheduler configuration.
pub fn init_system(config: &NetbricksConfiguration) {
    if config.name.is_empty() {
        panic!("Configuration must provide a name.");
    }
    // We init with all devices blacklisted and rely on the attach logic to white list them as necessary.
    if config.secondary {
        // We do not have control over any of the other settings in this case.
        init_system_secondary(&config.name[..], config.lcore_mask(), config.primary_core);
    } else {
        init_system_wl_with_mempool(
            &config.name[..],
            config.lcore_mask(),
            config.primary_core,
            &[],
            config.pool_size,
            config.cache_size,
            &config.vdevs,
        );
    }
    set_numa_domain();
}

thread_local!(static NUMA_DOMAIN: Cell<i32> = Cell::new(-1));

fn set_numa_domain() {
    let domain = unsafe {
        if libnuma::numa_available() == -1 {
            info!("No NUMA information found, support disabled");
            -1
        } else {
            let domain = libnuma::numa_preferred();
            info!("Running on numa node {}", domain);
            domain
        }
    };
    NUMA_DOMAIN.with(|f| f.set(domain))
}

/// Affinitize a pthread to a core and assign a DPDK thread ID.
pub fn init_thread(tid: i32, core: i32) {
    let numa = unsafe { zcsi::init_thread(tid, core) };
    NUMA_DOMAIN.with(|f| { f.set(numa); });
    if numa == -1 {
        info!("No NUMA information found, support disabled");
    } else {
        info!("Running on numa node {}", numa);
    };
}

#[inline]
pub fn get_domain() -> i32 {
    NUMA_DOMAIN.with(|f| f.get())
}
