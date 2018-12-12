#![allow(non_camel_case_types)]
use std::fmt;
use std::ptr;
use super::ol_flags::*;

/* automatically generated by rust-bindgen from mbuf.h */
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct __BindgenBitfieldUnit<Storage, Align>
    where
        Storage: AsRef<[u8]> + AsMut<[u8]>,
{
    storage: Storage,
    align: [Align; 0],
}
impl<Storage, Align> __BindgenBitfieldUnit<Storage, Align>
    where
        Storage: AsRef<[u8]> + AsMut<[u8]>,
{
    #[inline]
    pub fn new(storage: Storage) -> Self {
        Self { storage, align: [] }
    }
    #[inline]
    pub fn get_bit(&self, index: usize) -> bool {
        debug_assert!(index / 8 < self.storage.as_ref().len());
        let byte_index = index / 8;
        let byte = self.storage.as_ref()[byte_index];
        let bit_index = if cfg!(target_endian = "big") {
            7 - (index % 8)
        } else {
            index % 8
        };
        let mask = 1 << bit_index;
        byte & mask == mask
    }
    #[inline]
    pub fn set_bit(&mut self, index: usize, val: bool) {
        debug_assert!(index / 8 < self.storage.as_ref().len());
        let byte_index = index / 8;
        let byte = &mut self.storage.as_mut()[byte_index];
        let bit_index = if cfg!(target_endian = "big") {
            7 - (index % 8)
        } else {
            index % 8
        };
        let mask = 1 << bit_index;
        if val {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }
    #[inline]
    pub fn get(&self, bit_offset: usize, bit_width: u8) -> u64 {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 < self.storage.as_ref().len());
        debug_assert!((bit_offset + (bit_width as usize)) / 8 <= self.storage.as_ref().len());
        let mut val = 0;
        for i in 0..(bit_width as usize) {
            if self.get_bit(i + bit_offset) {
                let index = if cfg!(target_endian = "big") {
                    bit_width as usize - 1 - i
                } else {
                    i
                };
                val |= 1 << index;
            }
        }
        val
    }
    #[inline]
    pub fn set(&mut self, bit_offset: usize, bit_width: u8, val: u64) {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 < self.storage.as_ref().len());
        debug_assert!((bit_offset + (bit_width as usize)) / 8 <= self.storage.as_ref().len());
        for i in 0..(bit_width as usize) {
            let mask = 1 << i;
            let val_bit_is_set = val & mask == mask;
            let index = if cfg!(target_endian = "big") {
                bit_width as usize - 1 - i
            } else {
                i
            };
            self.set_bit(index + bit_offset, val_bit_is_set);
        }
    }
}
pub const _STDINT_H: u32 = 1;
pub const _FEATURES_H: u32 = 1;
pub const __USE_ANSI: u32 = 1;
pub const _BSD_SOURCE: u32 = 1;
pub const _SVID_SOURCE: u32 = 1;
pub const __USE_ISOC11: u32 = 1;
pub const __USE_ISOC99: u32 = 1;
pub const __USE_ISOC95: u32 = 1;
pub const _POSIX_SOURCE: u32 = 1;
pub const _POSIX_C_SOURCE: u32 = 200809;
pub const __USE_POSIX_IMPLICITLY: u32 = 1;
pub const __USE_POSIX: u32 = 1;
pub const __USE_POSIX2: u32 = 1;
pub const __USE_POSIX199309: u32 = 1;
pub const __USE_POSIX199506: u32 = 1;
pub const __USE_XOPEN2K: u32 = 1;
pub const __USE_XOPEN2K8: u32 = 1;
pub const _ATFILE_SOURCE: u32 = 1;
pub const __USE_MISC: u32 = 1;
pub const __USE_BSD: u32 = 1;
pub const __USE_SVID: u32 = 1;
pub const __USE_ATFILE: u32 = 1;
pub const __USE_FORTIFY_LEVEL: u32 = 0;
pub const _STDC_PREDEF_H: u32 = 1;
pub const __STDC_IEC_559__: u32 = 1;
pub const __STDC_IEC_559_COMPLEX__: u32 = 1;
pub const __STDC_ISO_10646__: u32 = 201103;
pub const __STDC_NO_THREADS__: u32 = 1;
pub const __GNU_LIBRARY__: u32 = 6;
pub const __GLIBC__: u32 = 2;
pub const __GLIBC_MINOR__: u32 = 17;
pub const __GLIBC_HAVE_LONG_LONG: u32 = 1;
pub const _SYS_CDEFS_H: u32 = 1;
pub const __WORDSIZE: u32 = 64;
pub const __WORDSIZE_TIME64_COMPAT32: u32 = 1;
pub const __SYSCALL_WORDSIZE: u32 = 64;
pub const _BITS_WCHAR_H: u32 = 1;
pub const __WCHAR_MIN: i32 = -2147483648;
pub const __WCHAR_MAX: u32 = 2147483647;
pub const INT8_MIN: i32 = -128;
pub const INT16_MIN: i32 = -32768;
pub const INT32_MIN: i32 = -2147483648;
pub const INT8_MAX: u32 = 127;
pub const INT16_MAX: u32 = 32767;
pub const INT32_MAX: u32 = 2147483647;
pub const UINT8_MAX: u32 = 255;
pub const UINT16_MAX: u32 = 65535;
pub const UINT32_MAX: u32 = 4294967295;
pub const INT_LEAST8_MIN: i32 = -128;
pub const INT_LEAST16_MIN: i32 = -32768;
pub const INT_LEAST32_MIN: i32 = -2147483648;
pub const INT_LEAST8_MAX: u32 = 127;
pub const INT_LEAST16_MAX: u32 = 32767;
pub const INT_LEAST32_MAX: u32 = 2147483647;
pub const UINT_LEAST8_MAX: u32 = 255;
pub const UINT_LEAST16_MAX: u32 = 65535;
pub const UINT_LEAST32_MAX: u32 = 4294967295;
pub const INT_FAST8_MIN: i32 = -128;
pub const INT_FAST16_MIN: i64 = -9223372036854775808;
pub const INT_FAST32_MIN: i64 = -9223372036854775808;
pub const INT_FAST8_MAX: u32 = 127;
pub const INT_FAST16_MAX: u64 = 9223372036854775807;
pub const INT_FAST32_MAX: u64 = 9223372036854775807;
pub const UINT_FAST8_MAX: u32 = 255;
pub const UINT_FAST16_MAX: i32 = -1;
pub const UINT_FAST32_MAX: i32 = -1;
pub const INTPTR_MIN: i64 = -9223372036854775808;
pub const INTPTR_MAX: u64 = 9223372036854775807;
pub const UINTPTR_MAX: i32 = -1;
pub const PTRDIFF_MIN: i64 = -9223372036854775808;
pub const PTRDIFF_MAX: u64 = 9223372036854775807;
pub const SIG_ATOMIC_MIN: i32 = -2147483648;
pub const SIG_ATOMIC_MAX: u32 = 2147483647;
pub const SIZE_MAX: i32 = -1;
pub const WCHAR_MIN: i32 = -2147483648;
pub const WCHAR_MAX: u32 = 2147483647;
pub const WINT_MIN: u32 = 0;
pub const WINT_MAX: u32 = 4294967295;
pub type int_least8_t = ::std::os::raw::c_schar;
pub type int_least16_t = ::std::os::raw::c_short;
pub type int_least32_t = ::std::os::raw::c_int;
pub type int_least64_t = ::std::os::raw::c_long;
pub type uint_least8_t = ::std::os::raw::c_uchar;
pub type uint_least16_t = ::std::os::raw::c_ushort;
pub type uint_least32_t = ::std::os::raw::c_uint;
pub type uint_least64_t = ::std::os::raw::c_ulong;
pub type int_fast8_t = ::std::os::raw::c_schar;
pub type int_fast16_t = ::std::os::raw::c_long;
pub type int_fast32_t = ::std::os::raw::c_long;
pub type int_fast64_t = ::std::os::raw::c_long;
pub type uint_fast8_t = ::std::os::raw::c_uchar;
pub type uint_fast16_t = ::std::os::raw::c_ulong;
pub type uint_fast32_t = ::std::os::raw::c_ulong;
pub type uint_fast64_t = ::std::os::raw::c_ulong;
pub type intmax_t = ::std::os::raw::c_long;
pub type uintmax_t = ::std::os::raw::c_ulong;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct L234len {
    pub _bitfield_1: __BindgenBitfieldUnit<[u8; 8usize], u16>,
    pub __bindgen_align: [u64; 0usize],
}
#[test]
fn bindgen_test_layout_L234len() {
    assert_eq!(
        ::std::mem::size_of::<L234len>(),
        8usize,
        concat!("Size of: ", stringify!(L234len))
    );
    assert_eq!(
        ::std::mem::align_of::<L234len>(),
        8usize,
        concat!("Alignment of ", stringify!(L234len))
    );
}
impl L234len {
    #[inline]
    pub fn l2_len(&self) -> u64 {
        unsafe { ::std::mem::transmute(self._bitfield_1.get(0usize, 7u8) as u64) }
    }
    #[inline]
    pub fn set_l2_len(&mut self, val: u64) {
        unsafe {
            let val: u64 = ::std::mem::transmute(val);
            self._bitfield_1.set(0usize, 7u8, val as u64)
        }
    }
    #[inline]
    pub fn l3_len(&self) -> u64 {
        unsafe { ::std::mem::transmute(self._bitfield_1.get(7usize, 9u8) as u64) }
    }
    #[inline]
    pub fn set_l3_len(&mut self, val: u64) {
        unsafe {
            let val: u64 = ::std::mem::transmute(val);
            self._bitfield_1.set(7usize, 9u8, val as u64)
        }
    }
    #[inline]
    pub fn l4_len(&self) -> u64 {
        unsafe { ::std::mem::transmute(self._bitfield_1.get(16usize, 8u8) as u64) }
    }
    #[inline]
    pub fn set_l4_len(&mut self, val: u64) {
        unsafe {
            let val: u64 = ::std::mem::transmute(val);
            self._bitfield_1.set(16usize, 8u8, val as u64)
        }
    }
    #[inline]
    pub fn tso_segsz(&self) -> u64 {
        unsafe { ::std::mem::transmute(self._bitfield_1.get(24usize, 16u8) as u64) }
    }
    #[inline]
    pub fn set_tso_segsz(&mut self, val: u64) {
        unsafe {
            let val: u64 = ::std::mem::transmute(val);
            self._bitfield_1.set(24usize, 16u8, val as u64)
        }
    }
    #[inline]
    pub fn outer_l3_len(&self) -> u64 {
        unsafe { ::std::mem::transmute(self._bitfield_1.get(40usize, 9u8) as u64) }
    }
    #[inline]
    pub fn set_outer_l3_len(&mut self, val: u64) {
        unsafe {
            let val: u64 = ::std::mem::transmute(val);
            self._bitfield_1.set(40usize, 9u8, val as u64)
        }
    }
    #[inline]
    pub fn outer_l2_len(&self) -> u64 {
        unsafe { ::std::mem::transmute(self._bitfield_1.get(49usize, 7u8) as u64) }
    }
    #[inline]
    pub fn set_outer_l2_len(&mut self, val: u64) {
        unsafe {
            let val: u64 = ::std::mem::transmute(val);
            self._bitfield_1.set(49usize, 7u8, val as u64)
        }
    }
    #[inline]
    pub fn new_bitfield_1(
        l2_len: u64,
        l3_len: u64,
        l4_len: u64,
        tso_segsz: u64,
        outer_l3_len: u64,
        outer_l2_len: u64,
    ) -> __BindgenBitfieldUnit<[u8; 8usize], u16> {
        let mut __bindgen_bitfield_unit: __BindgenBitfieldUnit<[u8; 8usize], u16> = Default::default();
        __bindgen_bitfield_unit.set(0usize, 7u8, {
            let l2_len: u64 = unsafe { ::std::mem::transmute(l2_len) };
            l2_len as u64
        });
        __bindgen_bitfield_unit.set(7usize, 9u8, {
            let l3_len: u64 = unsafe { ::std::mem::transmute(l3_len) };
            l3_len as u64
        });
        __bindgen_bitfield_unit.set(16usize, 8u8, {
            let l4_len: u64 = unsafe { ::std::mem::transmute(l4_len) };
            l4_len as u64
        });
        __bindgen_bitfield_unit.set(24usize, 16u8, {
            let tso_segsz: u64 = unsafe { ::std::mem::transmute(tso_segsz) };
            tso_segsz as u64
        });
        __bindgen_bitfield_unit.set(40usize, 9u8, {
            let outer_l3_len: u64 = unsafe { ::std::mem::transmute(outer_l3_len) };
            outer_l3_len as u64
        });
        __bindgen_bitfield_unit.set(49usize, 7u8, {
            let outer_l2_len: u64 = unsafe { ::std::mem::transmute(outer_l2_len) };
            outer_l2_len as u64
        });
        __bindgen_bitfield_unit
    }
}
#[repr(C)]
#[derive(Copy, Clone)]
pub union TxOffload {
    pub tx_offload: u64,
    pub l234len: L234len,
    _bindgen_union_align: u64,
}
#[test]
fn bindgen_test_layout_TxOffload() {
    assert_eq!(
        ::std::mem::size_of::<TxOffload>(),
        8usize,
        concat!("Size of: ", stringify!(TxOffload))
    );
    assert_eq!(
        ::std::mem::align_of::<TxOffload>(),
        8usize,
        concat!("Alignment of ", stringify!(TxOffload))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<TxOffload>())).tx_offload as *const _ as usize },
        0usize,
        concat!("Offset of field: ", stringify!(TxOffload), "::", stringify!(tx_offload))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<TxOffload>())).l234len as *const _ as usize },
        0usize,
        concat!("Offset of field: ", stringify!(TxOffload), "::", stringify!(l234len))
    );
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct MBuf {
    pub buf_addr: *mut u8,
    pub phys_addr: u64,
    pub data_off: u16,
    pub refcnt: u16,
    pub nb_segs: u16,
    pub port: u16,
    pub ol_flags: u64,
    pub packet_type: u32,
    pub pkt_len: u32,
    pub data_len: u16,
    pub vlan_tci: u16,
    pub hash_rss: u32,
    pub hash_hi: u32,
    pub vlan_tci_outer: u16,
    pub buf_len: u16,
    pub timestamp: u64,
    pub userdata: u64,
    pub pool: u64,
    pub next: *mut MBuf,
    pub tx_offload: TxOffload,
    pub priv_size: u16,
    pub timesync: u16,
    pub seqn: u32,
}
#[test]
fn bindgen_test_layout_MBuf() {
    assert_eq!(
        ::std::mem::size_of::<MBuf>(),
        104usize,
        concat!("Size of: ", stringify!(MBuf))
    );
    assert_eq!(
        ::std::mem::align_of::<MBuf>(),
        8usize,
        concat!("Alignment of ", stringify!(MBuf))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).buf_addr as *const _ as usize },
        0usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(buf_addr))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).phys_addr as *const _ as usize },
        8usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(phys_addr))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).data_off as *const _ as usize },
        16usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(data_off))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).refcnt as *const _ as usize },
        18usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(refcnt))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).nb_segs as *const _ as usize },
        20usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(nb_segs))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).port as *const _ as usize },
        22usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(port))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).ol_flags as *const _ as usize },
        24usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(ol_flags))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).packet_type as *const _ as usize },
        32usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(packet_type))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).pkt_len as *const _ as usize },
        36usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(pkt_len))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).data_len as *const _ as usize },
        40usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(data_len))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).vlan_tci as *const _ as usize },
        42usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(vlan_tci))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).hash_rss as *const _ as usize },
        44usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(hash_rss))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).hash_hi as *const _ as usize },
        48usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(hash_hi))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).vlan_tci_outer as *const _ as usize },
        52usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(vlan_tci_outer))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).buf_len as *const _ as usize },
        54usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(buf_len))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).timestamp as *const _ as usize },
        56usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(timestamp))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).userdata as *const _ as usize },
        64usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(userdata))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).pool as *const _ as usize },
        72usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(pool))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).next as *const _ as usize },
        80usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(next))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).tx_offload as *const _ as usize },
        88usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(tx_offload))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).priv_size as *const _ as usize },
        96usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(priv_size))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).timesync as *const _ as usize },
        98usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(timesync))
    );
    assert_eq!(
        unsafe { &(*(::std::ptr::null::<MBuf>())).seqn as *const _ as usize },
        100usize,
        concat!("Offset of field: ", stringify!(MBuf), "::", stringify!(seqn))
    );
}

/**** end of generated code ****/


/*
#[repr(C)]
#[derive(Copy, Clone)]
pub struct MBuf {
    buf_addr: *mut u8,
    phys_addr: usize,
    data_off: u16,
    refcnt: u16,
    nb_segs: u16, // now u16 from u8
    port: u16,    // now u16 from u8
    // still 4 bytes available here in first cacheline
    ol_flags: u64,
    packet_type: u32,
    pkt_len: u32,
    data_len: u16,
    vlan_tci: u16,
    hash_rss: u32,
    hash_hi: u32,
    //    seqn: u32, moved down
    vlan_tci_outer: u16, // now u16 from u32
    buf_len: u16,        //  /**< Length of segment buffer. */
    timestamp: u64,      // new
    // here starts the second cacheline
    userdata: u64,
    pool: u64,
    next: *mut MBuf,
    pub tx_offload: u64,
    priv_size: u16,
    timesync: u16,
    seqn: u32, // /** Sequence number. See also rte_reorder_insert(). */
}
*/

// TODO: Remove this once we start using these functions correctly
#[allow(dead_code)]
impl MBuf {
    #[inline]
    pub fn read_metadata_slot(mbuf: *mut MBuf, slot: usize) -> usize {
        unsafe {
            let ptr = (mbuf.offset(1) as *mut usize).offset(slot as isize);
            *ptr
        }
    }

    #[inline]
    pub fn write_metadata_slot(mbuf: *mut MBuf, slot: usize, value: usize) {
        unsafe {
            let ptr = (mbuf.offset(1) as *mut usize).offset(slot as isize);
            *ptr = value;
        }
    }

    #[inline]
    pub unsafe fn metadata_as<T: Sized>(mbuf: *const MBuf, slot: usize) -> *const T {
        (mbuf.offset(1) as *const usize).offset(slot as isize) as *const T
    }

    #[inline]
    pub unsafe fn mut_metadata_as<T: Sized>(mbuf: *mut MBuf, slot: usize) -> *mut T {
        (mbuf.offset(1) as *mut usize).offset(slot as isize) as *mut T
    }

    #[inline]
    pub fn data_address(&self, offset: usize) -> *mut u8 {
        unsafe { self.buf_addr.offset(self.data_off as isize + offset as isize) }
    }

    /// Returns the total allocated size of this mbuf segment.
    /// This is a constant.
    #[inline]
    pub fn buf_len(&self) -> usize {
        self.buf_len as usize
    }

    /// Returns the length of data in this mbuf segment.
    #[inline]
    pub fn data_len(&self) -> usize {
        self.data_len as usize
    }

    /// Returns the size of the packet (across multiple mbuf segment).
    #[inline]
    pub fn pkt_len(&self) -> usize {
        self.pkt_len as usize
    }

    #[inline]
    fn pkt_headroom(&self) -> usize {
        self.data_off as usize
    }

    #[inline]
    pub fn pkt_tailroom(&self) -> usize {
        self.buf_len() - self.data_off as usize - self.data_len()
    }

    /// Add data to the beginning of the packet. This might fail (i.e., return 0) when no more headroom is left.
    #[inline]
    pub fn add_data_beginning(&mut self, len: usize) -> usize {
        // If only we could add a likely here.
        if len > self.pkt_headroom() {
            0
        } else {
            self.data_off -= len as u16;
            self.data_len += len as u16;
            self.pkt_len += len as u32;
            len
        }
    }

    /// Add data to the end of a packet buffer. This might fail (i.e., return 0) when no more tailroom is left. We do
    /// not currently deal with packet with multiple segments.
    #[inline]
    pub fn add_data_end(&mut self, len: usize) -> usize {
        if len > self.pkt_tailroom() {
            0
        } else {
            self.data_len += len as u16;
            self.pkt_len += len as u32;
            len
        }
    }

    #[inline]
    pub fn remove_data_beginning(&mut self, len: usize) -> usize {
        if len > self.data_len() {
            0
        } else {
            self.data_off += len as u16;
            self.data_len -= len as u16;
            self.pkt_len -= len as u32;
            len
        }
    }

    #[inline]
    pub fn remove_data_end(&mut self, len: usize) -> usize {
        if len > self.data_len() {
            0
        } else {
            self.data_len -= len as u16;
            self.pkt_len -= len as u32;
            len
        }
    }

    #[inline]
    pub fn refcnt(&self) -> u16 {
        self.refcnt
    }

    #[inline]
    pub fn reference(&mut self) {
        self.refcnt += 1;
    }

    #[inline]
    pub fn dereference(&mut self) {
        self.refcnt -= 1;
    }

    #[inline]
    pub fn set_refcnt(&mut self, new_value: u16) {
        self.refcnt = new_value;
    }

    // copy payload and selected fields to target tmb
    #[inline]
    pub fn copy_to(&self, tmb: &mut MBuf) {
        (*tmb).data_len = (*self).data_len;
        (*tmb).data_off = (*self).data_off;
        (*tmb).pkt_len = (*self).pkt_len;
        unsafe { ptr::copy_nonoverlapping(self.data_address(0), (*tmb).data_address(0), self.data_len()); }
    }

    #[inline]
    pub fn set_tcp_ipv4_checksum_tx_offload(&mut self) {
        self.ol_flags |= PKT_TX_IPV4 | PKT_TX_IP_CKSUM | PKT_TX_TCP_CKSUM;
    }

    #[inline]
    pub fn ipv4_checksum_tx_offload(&mut self) -> bool {
        self.ol_flags & PKT_TX_IPV4 != 0 && self.ol_flags & PKT_TX_IP_CKSUM != 0
    }

    #[inline]
    pub fn tcp_checksum_tx_offload(&mut self) -> bool {
        self.ol_flags & PKT_TX_TCP_CKSUM != 0
    }

    #[inline]
    pub fn l2_len(&self) -> u64 {
        unsafe { self.tx_offload.l234len.l2_len() }
    }
    #[inline]
    pub fn set_l2_len(&mut self, val: u64) {
        unsafe { self.tx_offload.l234len.set_l2_len(val); }
    }

    #[inline]
    pub fn l3_len(&self) -> u64 {
        unsafe { self.tx_offload.l234len.l3_len() }
    }
    #[inline]
    pub fn set_l3_len(&mut self, val: u64) {
        unsafe { self.tx_offload.l234len.set_l3_len(val); }
    }

    #[inline]
    pub fn l4_len(&self) -> u64 {
        unsafe { self.tx_offload.l234len.l4_len() }
    }
    #[inline]
    pub fn set_l4_len(&mut self, val: u64) {
        unsafe { self.tx_offload.l234len.set_l4_len(val); }
    }
}

impl fmt::Display for MBuf {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "MBuf(&buf_addr= {:p}, data_len= {}, refcnt= {}, data_off= {}, data=",
            self.buf_addr,
            self.data_len(),
            self.refcnt(),
            self.data_off,
        )?;
        for i in 0.. self.data_len { write!(f, "{:x}", unsafe {*self.data_address(i as usize)} )?; }
        Ok(())
    }
}
