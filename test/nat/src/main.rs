#![feature(box_syntax)]
extern crate e2d2;
extern crate fnv;
extern crate getopts;
extern crate rand;
extern crate time;
extern crate uuid;

use uuid::Uuid;
use self::nf::*;
use e2d2::config::{basic_opts, read_matches};
use e2d2::interface::*;
use e2d2::operators::*;
use e2d2::scheduler::*;
use e2d2::allocators::CacheAligned;
use std::collections::HashSet;
use std::env;
use std::fmt::Display;
use std::net::Ipv4Addr;
use std::process;
use std::thread;
use std::time::Duration;
mod nf;

const CONVERSION_FACTOR: f64 = 1000000000.;

fn test<T, S>(ports: HashSet<T>, sched: &mut S)
where
    T: PacketRx + PacketTx + Display + Clone + Eq + std::hash::Hash + 'static,
    S: Scheduler + Sized,
{
    println!("Receiving started");

    let mut pipelines: Vec<_> = ports
        .iter()
        .map(|port| nat(ReceiveBatch::new(port.clone()), sched, &Ipv4Addr::new(10, 0, 0, 1)).send(port.clone()))
        .collect();
    println!("Running {} pipelines", pipelines.len());
    let uuid = Uuid::new_v4();
    let name = String::from("pipeline");
    if pipelines.len() > 1 {
        sched.add_runnable(Runnable::from_task(uuid, name, merge(pipelines)).move_ready());
    } else {
        sched.add_runnable(Runnable::from_task(uuid, name, pipelines.pop().unwrap()).move_ready());
    };
}

fn main() {
    let opts = basic_opts();

    let args: Vec<String> = env::args().collect();
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };
    let mut configuration = read_matches(&matches, &opts);

    match initialize_system(&mut configuration) {
        Ok(mut context) => {
            context.start_schedulers();
            context.add_pipeline_to_run(Box::new(move |_core: i32, p: HashSet<CacheAligned<PortQueue>>, s: &mut StandaloneScheduler| {
                test(p, s)
            } ));
            context.execute();

            let mut pkts_so_far = (0, 0);
            let mut start = time::precise_time_ns() as f64 / CONVERSION_FACTOR;
            let sleep_time = Duration::from_millis(500);
            loop {
                thread::sleep(sleep_time); // Sleep for a bit
                let now = time::precise_time_ns() as f64 / CONVERSION_FACTOR;
                if now - start > 1.0 {
                    let mut rx = 0;
                    let mut tx = 0;
                    for port in context.ports.values() {
                        for q in 0..port.rxqs() {
                            let (rp, tp, _q_len) = port.stats(q);
                            rx += rp;
                            tx += tp;
                        }
                    }
                    let pkts = (rx, tx);
                    println!(
                        "{:.2} OVERALL RX {:.2} TX {:.2}",
                        now - start,
                        (pkts.0 - pkts_so_far.0) as f64 / (now - start),
                        (pkts.1 - pkts_so_far.1) as f64 / (now - start)
                    );
                    start = now;
                    pkts_so_far = pkts;
                }
            }
        }
        Err(ref e) => {
            println!("Error: {}", e);
            if let Some(backtrace) = e.backtrace() {
                println!("Backtrace: {:?}", backtrace);
            }
            process::exit(1);
        }
    }
}
