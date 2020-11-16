#![feature(box_syntax)]
extern crate e2d2;
extern crate getopts;
extern crate rand;
extern crate time;
use e2d2::allocators::*;
use e2d2::interface::dpdk::*;
use e2d2::interface::*;
use e2d2::operators::*;
use e2d2::scheduler::Executable;
use e2d2::state::*;
use getopts::Options;
use std::collections::HashMap;
use std::env;
use std::process;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const CONVERSION_FACTOR: f64 = 1000000000.;

fn monitor<T: 'static + Batch>(
    parent: T,
    mut monitoring_cache: MergeableStoreDP<isize>,
) -> TransformBatch<TransformBatch<T>> {
    parent
        .transform(box |pkt| {
            let hdr = pkt.headers_mut().mac_mut(0);
            hdr.swap_addresses();
        })
        .transform(box move |pkt| {
            let hdr = pkt.headers_mut().ip_mut(1);
            let ttl = hdr.ttl();
            hdr.set_ttl(ttl + 1);
            monitoring_cache.update(hdr.flow().unwrap(), 1);
        })
}

fn recv_thread(ports: Vec<CacheAligned<PortQueue>>, core: i32, counter: MergeableStoreDP<isize>) {
    init_thread(core, core);
    println!("Receiving started");

    let pipelines: Vec<_> = ports
        .iter()
        .map(|port| {
            let ctr = counter.clone();
            box monitor(ReceiveBatch::new(port.clone()), ctr).send(port.clone()) as Box<dyn Batch>
        })
        .collect();
    println!("Running {} pipelines", pipelines.len());
    let mut combined = merge_batches(pipelines);
    loop {
        combined.execute();
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optflag("", "secondary", "run as a secondary process");
    opts.optopt("n", "name", "name to use for the current process", "name");
    opts.optmulti("p", "port", "Port to use", "[type:]id");
    opts.optmulti("c", "core", "Core to use", "core");
    opts.optopt("m", "master", "Master core", "master");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };
    if matches.opt_present("h") {
        print!("{}", opts.usage(&format!("Usage: {} [options]", program)));
        process::exit(0)
    }

    let cores_str = matches.opt_strs("c");
    let master_core: i32 = matches
        .opt_str("m")
        .unwrap_or_else(|| String::from("0"))
        .parse()
        .expect("Could not parse master core spec");
    println!("Using master core {}", master_core);
    let name = matches.opt_str("n").unwrap_or_else(|| String::from("recv"));

    let cores: Vec<i32> = cores_str
        .iter()
        .map(|n: &String| n.parse().ok().expect(&format!("Core cannot be parsed {}", n)))
        .collect();

    fn extract_cores_for_port(ports: &[String], cores: &[i32]) -> HashMap<String, Vec<i32>> {
        let mut cores_for_port = HashMap::<String, Vec<i32>>::new();
        for (port, core) in ports.iter().zip(cores.iter()) {
            cores_for_port.entry(port.clone()).or_insert(vec![]).push(*core)
        }
        cores_for_port
    }

    let primary = !matches.opt_present("secondary");

    let cores_for_port = extract_cores_for_port(&matches.opt_strs("p"), &cores);
    let vdev_list = vec![];
    let lcore_mask: u64 = 1u64 << master_core;
    if primary {
        init_system_wl(&name, lcore_mask, master_core as i32, &[], &vdev_list);
    } else {
        init_system_secondary(&name, lcore_mask, master_core as i32);
    }

    let ports_to_activate: Vec<_> = cores_for_port.keys().collect();

    let mut queues_by_core = HashMap::<i32, Vec<_>>::with_capacity(cores.len());
    let mut ports = Vec::<Arc<PmdPort>>::with_capacity(ports_to_activate.len());
    for port in &ports_to_activate {
        let cores = cores_for_port.get(*port).unwrap();
        let queues = cores.len() as i32;
        let pmd_port =
            PmdPort::new_with_queues(*port, queues, queues, cores, cores).expect("Could not initialize port");
        for (idx, core) in cores.iter().enumerate() {
            let queue = idx as u16;
            queues_by_core
                .entry(*core)
                .or_insert(vec![])
                .push(PmdPort::new_queue_pair(&pmd_port, queue, queue).unwrap());
        }
        ports.push(pmd_port);
    }

    const _BATCH: usize = 1 << 10;
    const _CHANNEL_SIZE: usize = 256;
    let mut consumer = MergeableStoreCP::new();
    let _thread: Vec<_> = queues_by_core
        .iter()
        .map(|(core, ports)| {
            let c = core.clone();
            let mon = consumer.dp_store();
            let p: Vec<_> = ports.iter().map(|p| p.clone()).collect();
            std::thread::spawn(move || recv_thread(p, c, mon))
        })
        .collect();
    let mut pkts_so_far = (0, 0);
    let mut start = time::precise_time_ns() as f64 / CONVERSION_FACTOR;
    let sleep_time = Duration::from_millis(500);
    loop {
        thread::sleep(sleep_time); // Sleep for a bit
        consumer.sync();
        let now = time::precise_time_ns() as f64 / CONVERSION_FACTOR;
        if now - start > 1.0 {
            let mut rx = 0;
            let mut tx = 0;
            for port in &ports {
                for q in 0..port.rxqs() {
                    let (rp, tp, _q_len) = port.stats(q);
                    rx += rp;
                    tx += tp;
                }
            }
            let pkts = (rx, tx);
            println!(
                "{:.2} OVERALL RX {:.2} TX {:.2} FLOWS {}",
                now - start,
                (pkts.0 - pkts_so_far.0) as f64 / (now - start),
                (pkts.1 - pkts_so_far.1) as f64 / (now - start),
                consumer.len()
            );
            start = now;
            pkts_so_far = pkts;
        }
    }
}
