// copied from : https://github.com/libpnet/libpnet/blob/master/pnet_packet/src/util.rs
// modified by (C) 2017 Rainer Stademann
/*
Copyright (c) 2014-2016 Robert Clipsham
Copyright

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
*/
#![allow(non_camel_case_types)]

/// Represents an unsigned 16-bit integer. libpnet #[packet]-derived structs using this type will
/// hold it in memory as big-endian, but accessors/mutators will return/take host-order values.

pub type u16be = u16;

/// Sum all words (16 bit chunks) in the given data. The word at word offset
/// `skipword` will be skipped. Each word is treated as big endian.

use std::slice;


fn sum_be_words(data: &[u8], mut skipword: usize) -> u32 {
    let len = data.len();
    let wdata: &[u16] = unsafe { slice::from_raw_parts(data.as_ptr() as *const u16, len / 2) };
    skipword = ::std::cmp::min(skipword, wdata.len());

    let mut sum = 0u32;
    let mut i = 0;
    while i < skipword {
        sum += u16::from_be(unsafe { *wdata.get_unchecked(i) }) as u32;
        i += 1;
    }
    i += 1;
    while i < wdata.len() {
        sum += u16::from_be(unsafe { *wdata.get_unchecked(i) }) as u32;
        i += 1;
    }
    // If the length is odd, make sure to checksum the final byte
    if len & 1 != 0 {
        sum += (unsafe { *data.get_unchecked(len - 1) } as u32) << 8;
    }

    sum
}

fn sum_be_words_ptr(data: *mut u8, len: usize, mut skipword: usize) -> u32 {
    let wdata: &[u16] = unsafe { slice::from_raw_parts(data as *const u16, len / 2) };
    skipword = ::std::cmp::min(skipword, wdata.len());

    let mut sum = 0u32;
    let mut i = 0;
    while i < skipword {
        sum += u16::from_be(unsafe { *wdata.get_unchecked(i) }) as u32;
        i += 1;
    }
    i += 1;
    while i < wdata.len() {
        sum += u16::from_be(unsafe { *wdata.get_unchecked(i) }) as u32;
        i += 1;
    }
    // If the length is odd, make sure to checksum the final byte
    if len & 1usize != 0 {
        sum += (unsafe { *data.offset((len - 1) as isize) } as u32) << 8;
    }

    sum
}

/// Calculates a checksum. Used by ipv4 and icmp. The two bytes starting at `skipword * 2` will be
/// ignored. Supposed to be the checksum field, which is regarded as zero during calculation.
pub fn checksum(data: &[u8], skipword: usize) -> u16be {
    let sum = sum_be_words(data, skipword);
    finalize_checksum(sum)
}

pub fn finalize_checksum(mut sum: u32) -> u16be {
    while sum >> 16 != 0 {
        sum = (sum >> 16) + (sum & 0xFFFF);
    }
    !sum as u16
}

/// Calculate the checksum for a packet built on IPv4. Used by udp and tcp.
pub fn ipv4_checksum(
    data: *mut u8,
    len: usize,
    skipword: usize,
    extra_data: &[u8],
    src_ip: u32,
    dst_ip: u32,
    next_level_protocol: u32,
) -> u16be {
    let mut sum = 0u32;

    // Checksum pseudo-header
    //sum += ipv4_word_sum(source);
    //sum += ipv4_word_sum(destination);
    sum += !finalize_checksum(src_ip) as u32;
    sum += !finalize_checksum(dst_ip) as u32;

    sum += next_level_protocol;

    let len = len + extra_data.len();
    sum += len as u32;

    // Checksum packet header and data
    sum += sum_be_words_ptr(data, len, skipword);
    sum += sum_be_words(extra_data, extra_data.len() / 2);

    finalize_checksum(sum)
}

// everything in host byte order:
pub fn update_checksum_incremental(old_check: u16, old_data_csum: u16, new_data_csum: u16) -> u16be {
    let tmp: u32;
    tmp = (!old_check) as u32 + (!old_data_csum) as u32 + new_data_csum as u32;
    finalize_checksum(tmp)
}
