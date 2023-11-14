extern crate e2d2;
extern crate fnv;
extern crate getopts;
extern crate rand;
extern crate time;
extern crate uuid;

use self::nf::*;
use e2d2::allocators::CacheAligned;
use e2d2::config::*;
use e2d2::interface::*;
use e2d2::operators::*;
use e2d2::scheduler::*;
use std::collections::HashSet;
use std::env;
use std::thread::sleep;
use std::time::Duration;
use uuid::Uuid;

mod nf;

fn test<S: Scheduler + Sized>(ports: HashSet<CacheAligned<PortQueue>>, sched: &mut S) {
    let pipelines: Vec<_> = ports
        .iter()
        .map(|port| reconstruction(ReceiveBatch::new(port.clone()), sched).send(port.clone()))
        .collect();
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
    configuration.pool_size = 256; // Travis allows 512 hugepages, but reliably continguously produces 256.

    let mut config = initialize_system(&mut configuration).unwrap();

    config.start_schedulers();

    config.add_pipeline_to_run(Box::new(
        move |_core: i32, p: HashSet<CacheAligned<PortQueue>>, s: &mut StandaloneScheduler| test(p, s),
    ));
    println!("BEGIN TEST OUTPUT");
    config.execute();

    sleep(Duration::from_secs(10));
}
