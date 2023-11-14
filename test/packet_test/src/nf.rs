use e2d2::headers::*;
use e2d2::operators::*;
use eui48::MacAddress;

pub fn delay<T: 'static + Batch>(parent: T) -> TransformBatch<T> {
    let mut m = MacHeader::new();
    m.dst = MacAddress::new([0x68, 0x05, 0xca, 0x33, 0xff, 0x79]);
    m.src = MacAddress::new([0x68, 0x05, 0xca, 0x33, 0xfd, 0xc8]);
    m.set_etype(0x800);
    parent.transform(Box::new(move |pkt| {
        pkt.replace_header(0, &Header::new(&mut m as *mut MacHeader));
    }))
    // parent.parse::<MacHeader>()
    // .transform(box move |pkt| {
    // assert!(pkt.refcnt() == 1);
    // let mut hdr = pkt.get_mut_header();
    // /let src = hdr.src;
    // hdr.src[2] += 1;
    // hdr.dst[1] += 1;
    // delay_loop(delay);
    // })
}
