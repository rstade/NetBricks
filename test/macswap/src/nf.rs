use e2d2::operators::*;

pub fn macswap<T: 'static + Batch>(
    parent: T,
) -> TransformBatch<T> {
    parent.transform(box move |pkt| {
        assert!(pkt.refcnt() == 1);
        let hdr = pkt.headers_mut().mac_mut(0);
        hdr.swap_addresses();
    })
}
