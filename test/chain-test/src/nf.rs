use e2d2::operators::*;

#[inline]
pub fn chain_nf<T: 'static + Batch>(parent: T) ->  CompositionBatch {
    let next= parent
        .transform(box move |pkt| {
            let hdr = pkt.get_header_mut(0).as_mac().unwrap();
            hdr.swap_addresses();
        })
        .transform(box |pkt| {
            let h = pkt.get_header_mut(1).as_ip().unwrap();
            let ttl = h.ttl();
            h.set_ttl(ttl - 1);
        })
        .filter(box |pkt| {
            let h = pkt.get_header(1).as_ip().unwrap();
            h.ttl() != 0
        });
    CompositionBatch::new(next)
}

#[inline]
pub fn chain<S: 'static + Batch>(
    parent: S,
    len: u32,
    pos: u32,
) -> CompositionBatch {
    let mut chained = chain_nf(parent);
    for _ in 1..len {
        chained = chain_nf(chained);
    }
    let next= if len % 2 == 0 || pos % 2 == 1 {
        CompositionBatch::new(chained
            .transform(box move |pkt| {
                let hdr = pkt.get_header_mut(0).as_mac().unwrap();
                hdr.swap_addresses();
            }))
    } else {
        chained
    };

    CompositionBatch::new(next)
}
