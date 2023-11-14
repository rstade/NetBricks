extern crate e2d2;
extern crate eui48;
extern crate fnv;
extern crate getopts;
extern crate rand;
extern crate time;
extern crate uuid;

use self::nf::*;
use e2d2::allocators::CacheAligned;
use e2d2::config::{basic_opts, read_matches};
use e2d2::interface::*;
use e2d2::operators::*;
use e2d2::scheduler::*;
use std::collections::HashSet;
use std::env;
use std::fmt::Display;
use std::process;
use std::thread;
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

mod nf;

const CONVERSION_FACTOR: f64 = 1000000000.;

fn test<T, S>(ports: HashSet<T>, sched: &mut S)
where
    T: PacketRx + PacketTx + Display + Clone + Eq + std::hash::Hash + 'static,
    S: Scheduler + Sized,
{
    println!("Receiving started");

    let pipelines: Vec<_> = ports
        .iter()
        .map(|port| delay(ReceiveBatch::new(port.clone())).send(port.clone()))
        .collect();
    println!("Running {} pipelines", pipelines.len());
    for pipeline in pipelines {
        let uuid = Uuid::new_v4();
        let name = String::from("pipeline");
        sched.add_runnable(Runnable::from_task(uuid, name, pipeline).move_ready());
    }
}

fn main() {
    let opts = basic_opts();

    let args: Vec<String> = env::args().collect();
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!("{}", f.to_string()),
    };
    let mut configuration = read_matches(&matches, &opts);

    match initialize_system(&mut configuration) {
        Ok(mut context) => {
            context.start_schedulers();
            context.add_pipeline_to_run(Box::new(
                move |_core: i32, p: HashSet<CacheAligned<PortQueue>>, s: &mut StandaloneScheduler| test(p, s),
            ));
            context.execute();

            let mut pkts_so_far = (0, 0);
            let mut last_printed = 0.;
            const MAX_PRINT_INTERVAL: f64 = 30.;
            const PRINT_DELAY: f64 = 15.;
            let sleep_delay = (PRINT_DELAY / 2.) as u64;
            let mut start = OffsetDateTime::now_utc().unix_timestamp_nanos() as f64 / CONVERSION_FACTOR;
            let sleep_time = Duration::from_millis(sleep_delay);
            println!("0 OVERALL RX 0.00 TX 0.00 CYCLE_PER_DELAY 0 0 0");
            loop {
                thread::sleep(sleep_time); // Sleep for a bit
                let now = OffsetDateTime::now_utc().unix_timestamp_nanos() as f64 / CONVERSION_FACTOR;
                if now - start > PRINT_DELAY {
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
                    let rx_pkts = pkts.0 - pkts_so_far.0;
                    if rx_pkts > 0 || now - last_printed > MAX_PRINT_INTERVAL {
                        println!(
                            "{:.2} OVERALL RX {:.2} TX {:.2}",
                            now - start,
                            rx_pkts as f64 / (now - start),
                            (pkts.1 - pkts_so_far.1) as f64 / (now - start)
                        );
                        last_printed = now;
                        start = now;
                        pkts_so_far = pkts;
                    }
                }
            }
        }
        Err(ref e) => {
            println!("Error: {}", e);
            process::exit(1);
        }
    }
}
