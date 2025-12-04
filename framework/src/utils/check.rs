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
#[inline]
fn sum_be_words(data: &[u8], mut skipword: usize) -> u32 {
    let len = data.len();
    // Process 16-bit big-endian words without assuming alignment of `data`.
    let mut sum = 0u32;
    let mut i = 0usize;

    let mut chunks = data.chunks_exact(2);
    while let Some(chunk) = chunks.next() {
        if i != skipword {
            // chunk has length 2
            let word = u16::from_be_bytes([chunk[0], chunk[1]]);
            sum += word as u32;
        }
        i += 1;
    }

    // If the length is odd, make sure to checksum the final byte
    if let [last] = chunks.remainder() {
        sum += (*last as u32) << 8;
    }

    sum
}

#[inline]
fn sum_be_words_ptr(data: *mut u8, len: usize, mut skipword: usize) -> u32 {
    // Create a temporary u8 slice (u8 has alignment 1, so this is always valid)
    let bytes: &[u8] = unsafe { slice::from_raw_parts(data as *const u8, len) };
    // Reuse the safe implementation above
    sum_be_words(bytes, skipword)
}

/// Calculates a checksum. Used by ipv4 and icmp. The two bytes starting at `skipword * 2` will be
/// ignored. Supposed to be the checksum field, which is regarded as zero during calculation.
#[inline]
pub fn checksum(data: &[u8], skipword: usize) -> u16be {
    let sum = sum_be_words(data, skipword);
    finalize_checksum(sum)
}

#[inline]
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

    // Total length includes payload in pseudo-header sum
    let total_len = len + extra_data.len();
    sum += total_len as u32;

    // Checksum packet header and data
    // Only the base buffer pointed to by `data` is covered by `len` bytes.
    sum += sum_be_words_ptr(data, len, skipword);
    sum += sum_be_words(extra_data, extra_data.len() / 2);

    finalize_checksum(sum)
}

// everything in host byte order:
#[inline]
pub fn update_checksum_incremental(old_check: u16, old_data_csum: u16, new_data_csum: u16) -> u16be {
    let tmp: u32;
    tmp = (!old_check) as u32 + (!old_data_csum) as u32 + new_data_csum as u32;
    finalize_checksum(tmp)
}
