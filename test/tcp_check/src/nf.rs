use e2d2::headers::*;
use e2d2::operators::*;

#[inline]
pub fn tcp_nf<T: 'static + Batch>(parent: T) -> CompositionBatch {
    parent
        .map(box |pkt| {
            println!("hdr {}", pkt.headers());
            let payload = pkt.get_payload(0);
            print!("Payload: ");
            for p in payload {
                print!("{:x} ", p);
            }
            println!();
        })
        .map(box |pkt| {
            let hdr = pkt.headers().ip(1);
            let flow = hdr.flow().unwrap();
            let payload = pkt.get_payload(1);
            println!("hdr {} ihl {} offset {}", hdr, hdr.ihl(), hdr.offset());
            println!(
                "payload: {:x} {:x} {:x} {:x}",
                payload[0], payload[1], payload[2], payload[3]
            );
            unsafe {
                println!("Src {} dst {}", flow.src_port, flow.dst_port);
            }
        })
        .map(box |pkt| {
            println!("TCP header {}", pkt.headers().tcp(2));
        })
        .compose()
}
