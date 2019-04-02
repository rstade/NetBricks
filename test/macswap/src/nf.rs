use e2d2::operators::*;

pub fn macswap<T: 'static + Batch>(
    parent: T,
) -> TransformBatch<T> {
    parent.transform(box move |pkt| {
        assert!(pkt.refcnt() == 1);
        let hdr = pkt.get_header_mut(0).as_mac().unwrap();
        hdr.swap_addresses();
    })
}
