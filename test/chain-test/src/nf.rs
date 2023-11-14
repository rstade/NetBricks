use e2d2::operators::*;

#[inline]
pub fn chain_nf<T: 'static + Batch>(parent: T) -> CompositionBatch {
    let next = parent
        .transform(Box::new(move |pkt| {
            let hdr = pkt.headers_mut().mac_mut(0);
            hdr.swap_addresses();
        }))
        .transform(Box::new(|pkt| {
            let h = pkt.headers_mut().ip_mut(1);
            let ttl = h.ttl();
            h.set_ttl(ttl - 1);
        }))
        .filter(Box::new(|pkt| {
            let h = pkt.headers().ip(1);
            h.ttl() != 0
        }));
    CompositionBatch::new(next)
}

#[inline]
pub fn chain<S: 'static + Batch>(parent: S, len: u32, pos: u32) -> CompositionBatch {
    let mut chained = chain_nf(parent);
    for _ in 1..len {
        chained = chain_nf(chained);
    }
    let next = if len % 2 == 0 || pos % 2 == 1 {
        CompositionBatch::new(chained.transform(Box::new(move |pkt| {
            let hdr = pkt.headers_mut().mac_mut(0);
            hdr.swap_addresses();
        })))
    } else {
        chained
    };

    CompositionBatch::new(next)
}
