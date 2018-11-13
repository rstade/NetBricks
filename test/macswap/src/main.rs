#![feature(box_syntax)]
#![feature(asm)]
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
use std::process;
use std::thread;
use std::time::Duration;

mod nf;

fn test<T, S>(ports: HashSet<T>, sched: &mut S)
    where
        T: PacketRx + PacketTx + Display + Clone + Eq + std::hash::Hash + 'static,
        S: Scheduler + Sized,
{
    for port in &ports {
        println!("Receiving port {}", port);
    }

    let pipelines: Vec<_> = ports
        .iter()
        .map(|port| macswap(ReceiveBatch::new(port.clone())).send(port.clone()))
        .collect();
    println!("Running {} pipelines", pipelines.len());
    for pipeline in pipelines {
        let uuid = Uuid::new_v4();
        let name = String::from("pipeline");
        sched.add_runnable(Runnable::from_task(uuid, name, pipeline).move_ready());
    }
}

fn main() {
    let mut opts = basic_opts();
    opts.optopt(
        "",
        "dur",
        "Test duration",
        "If this option is set to a nonzero value, then the \
         test will exit after X seconds.",
    );

    let args: Vec<String> = env::args().collect();
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };
    let mut configuration = read_matches(&matches, &opts);
    configuration.pool_size = 255;

    let test_duration: u64 = matches
        .opt_str("dur")
        .unwrap_or_else(|| String::from("0"))
        .parse()
        .expect("Could not parse test duration");

    match initialize_system(&mut configuration) {
        Ok(mut context) => {
            context.start_schedulers();

            context.add_pipeline_to_run(Box::new(move |_core: i32, p: HashSet<CacheAligned<PortQueue>>, s: &mut StandaloneScheduler| {
                test(p, s)
            })
            );
            context.execute();

            if test_duration != 0 {
                thread::sleep(Duration::from_secs(test_duration));
            } else {
                loop {
                    thread::sleep(Duration::from_secs(1));
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
