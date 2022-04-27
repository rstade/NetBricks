use e2d2::operators::*;
use std::arch::asm;

#[inline]
fn lat() {
    unsafe {
        asm!("nop");
    }
}

#[inline]
fn delay_loop(delay: u64) {
    let mut d = 0;
    while d < delay {
        lat();
        d += 1;
    }
}

pub fn delay<T: 'static + Batch>(parent: T, delay: u64) -> TransformBatch<T> {
    parent.transform(box move |pkt| {
        assert!(pkt.refcnt() == 1);
        let hdr = pkt.headers_mut().mac_mut(0);
        hdr.swap_addresses();
        delay_loop(delay);
    })
}
