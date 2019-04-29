use allocators::CacheAligned;
use common::{errors, ErrorKind};
use config::NetbricksConfiguration;
use interface::dpdk::{init_system, init_thread};
use interface::{PmdPort, PortQueue, VirtualPort, VirtualQueue};
use scheduler::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle, Thread};

type AlignedPortQueue = CacheAligned<PortQueue>;
type AlignedVirtualQueue = CacheAligned<VirtualQueue>;

/// A handle to schedulers paused on a barrier.
pub struct BarrierHandle<'a> {
    threads: Vec<&'a Thread>,
}

impl<'a> BarrierHandle<'a> {
    /// Release all threads. This consumes the handle as expected.
    pub fn release(self) {
        for thread in &self.threads {
            thread.unpark();
        }
    }

    /// Allocate a new BarrierHandle with threads.
    pub fn with_threads(threads: Vec<&'a Thread>) -> BarrierHandle {
        BarrierHandle { threads: threads }
    }
}

/// `NetBricksContext` contains handles to all schedulers, and provides mechanisms for coordination.
#[derive(Default)]
pub struct NetBricksContext {
    pub ports: HashMap<String, Arc<PmdPort>>,
    pub id_to_port: HashMap<u16, Arc<PmdPort>>,
    pub rx_queues: HashMap<i32, HashSet<CacheAligned<PortQueue>>>,
    // queues running on a core
    pub active_cores: Vec<i32>,
    pub virtual_ports: HashMap<i32, Arc<VirtualPort>>,
    pub scheduler_channels: HashMap<i32, SyncSender<SchedulerCommand>>,
    pub reply_receiver: Option<Receiver<SchedulerReply>>,
    scheduler_handles: HashMap<i32, JoinHandle<()>>,
}

impl NetBricksContext {
    /// Boot up all schedulers.
    pub fn start_schedulers(&mut self) {
        let cores = self.active_cores.clone();
        let (reply_sender, reply_receiver) = channel::<SchedulerReply>();
        self.reply_receiver = Some(reply_receiver);
        for core in &cores {
            self.init_scheduler(*core, reply_sender.clone());
        }
    }

    #[inline]
    fn init_scheduler(&mut self, core: i32, reply_sender: Sender<SchedulerReply>) {
        debug!("init scheduler on core-{}", core);
        let builder = thread::Builder::new();
        let (sender, receiver) = sync_channel(0);
        self.scheduler_channels.insert(core, sender);
        let join_handle = builder
            .name(format!("sched-{}", core).into())
            .spawn(move || {
                init_thread(core, core);
                // Other init?
                let mut sched = StandaloneScheduler::new_with_channel(core, receiver, reply_sender);
                sched.handle_requests()
            })
            .unwrap();
        self.scheduler_handles.insert(core, join_handle);
    }

    /// Run a function (which installs a pipeline) on all schedulers in the system.
    /// this function is deprecated and replaced by run_pipeline_on_cores, we only keep it for old test procedures
    pub fn add_pipeline_to_run<T>(&mut self, run: Box<T>)
    where
        T: Fn(i32, HashSet<AlignedPortQueue>, &mut StandaloneScheduler) + Send + Clone + 'static,
    {
        for (core, channel) in &self.scheduler_channels {
            let ports = match self.rx_queues.get(core) {
                Some(set) => set.clone(),
                None => HashSet::with_capacity(8),
            };

            let core_id = *core;
            let run_clone = run.clone();

            let closure = Box::new(move |s: &mut StandaloneScheduler| run_clone(core_id, ports.clone(), s));
            channel.send(SchedulerCommand::Run(closure)).unwrap();
        }
    }

    /// Run a function which installs pipelines on all schedulers in the system.
    /// it is up to the function "run" to detect which pipeline to install based on the hashmap of ports and the core_id
    pub fn install_pipeline_on_cores<T>(&mut self, run: Box<T>)
    where
        T: Fn(i32, HashMap<String, Arc<PmdPort>>, &mut StandaloneScheduler) + Send + Clone + 'static,
    {
        for (core, channel) in &self.scheduler_channels {
            let core_id = *core;
            let run_clone = run.clone();
            let ports = self.ports.clone();
            let closure = Box::new(move |s: &mut StandaloneScheduler| run_clone(core_id, ports.clone(), s));
            channel.send(SchedulerCommand::Run(closure)).unwrap();
        }
    }

    pub fn add_test_pipeline<S>(&mut self, run: Box<S>)
    where
        S: Fn(i32, Vec<AlignedVirtualQueue>, &mut StandaloneScheduler) + Send + Clone + 'static,
    {
        for (core, channel) in &self.scheduler_channels {
            let port = self.virtual_ports.entry(*core).or_insert(VirtualPort::new().unwrap());
            let queue = port.new_virtual_queue().unwrap();
            let core_id = *core;
            let run_clone = run.clone();
            let closure = Box::new(move |s: &mut StandaloneScheduler| run_clone(core_id, vec![queue.clone()], s));
            channel.send(SchedulerCommand::Run(closure)).unwrap();
        }
    }

    /// Make all pipelines ready and start scheduling.
    pub fn execute(&mut self) {
        for (core, channel) in &self.scheduler_channels {
            debug!("start executing scheduler on core-{}", core);
            channel.send(SchedulerCommand::SetTaskStateAll(true)).unwrap(); // this way we stay compatible with old code
            channel.send(SchedulerCommand::Execute).unwrap();
        }
    }

    /// Only start scheduling. Task states remain untouched.
    pub fn execute_schedulers(&mut self) {
        for (core, channel) in &self.scheduler_channels {
            debug!("start executing scheduler on core-{}", core);
            channel.send(SchedulerCommand::Execute).unwrap();
        }
    }

    /// Pause all schedulers, the returned `BarrierHandle` can be used to resume.
    pub fn barrier(&mut self) -> BarrierHandle {
        // TODO: If this becomes a problem, move this to the struct itself; but make sure to fix `stop` appropriately.
        let channels: Vec<_> = self.scheduler_handles.iter().map(|_| sync_channel(0)).collect();
        let receivers = channels.iter().map(|&(_, ref r)| r);
        let senders = channels.iter().map(|&(ref s, _)| s);
        for ((_, channel), sender) in self.scheduler_channels.iter().zip(senders) {
            channel.send(SchedulerCommand::Handshake(sender.clone())).unwrap();
        }
        for receiver in receivers {
            receiver.recv().unwrap();
        }
        BarrierHandle::with_threads(self.scheduler_handles.values().map(|j| j.thread()).collect())
    }

    /// Stop all schedulers, safely shutting down the system.
    pub fn stop(&mut self) {
        for (core, channel) in &self.scheduler_channels {
            channel.send(SchedulerCommand::Shutdown).unwrap();
            println!("Issued shutdown for core {}", core);
        }
        for (core, join_handle) in self.scheduler_handles.drain() {
            join_handle.join().unwrap();
            println!("Core {} has shutdown", core);
        }
        println!("System shutdown");
    }

    pub fn wait(&mut self) {
        for (core, join_handle) in self.scheduler_handles.drain() {
            join_handle.join().unwrap();
            println!("Core {} has shutdown", core);
        }
        println!("System shutdown");
    }

    /// Shutdown all schedulers.
    pub fn shutdown(&mut self) {
        self.stop()
    }
}

fn is_port_type_kni_or_virtio(name: &str) -> bool {
    let parts: Vec<_> = name.splitn(2, ':').collect();
    match parts[0] {
        "kni" => true,
        "virtio" => true,
        _ => false,
    }
}

/// Initialize the system from a configuration.
pub fn initialize_system(configuration: &mut NetbricksConfiguration) -> errors::Result<NetBricksContext> {
    init_system(configuration);
    let mut ctx: NetBricksContext = Default::default();
    let mut cores: HashSet<_> = configuration.cores.iter().cloned().collect();
    //maps kni name to port_id of associated port
    let mut kni2pci: HashMap<String, Arc<PmdPort>> = HashMap::with_capacity(configuration.ports.len());
    {
        let mut update_context: Box<FnMut(Arc<PmdPort>) -> Result<(), ErrorKind>> = Box::new(|p: Arc<PmdPort>| {
            info!("initialized {}", p);
            let port_id = p.port_id();
            if ctx.ports.contains_key(p.name()) {
                error!("Port {} appears twice in specification", p.name());
                Err(ErrorKind::ConfigurationError(format!("Port {} appears twice in specification", p.name())).into())
            } else {
                ctx.ports.insert(p.name().clone(), p.clone());
                if !ctx.id_to_port.contains_key(&port_id) {
                    ctx.id_to_port.insert(port_id, p);
                } else {
                    warn!("duplicate port_id = {}", port_id);
                }
                Ok(())
            }
        });

        // first we parse all ports which have a kni (either native Kni or Virtio) associated

        for port in &mut configuration.ports.iter_mut().filter(|p| p.kni.is_some()) {
            if is_port_type_kni_or_virtio(&port.name[..]) {
                error!(
                    "Port {} : native kni and virtio ports must not define an associated kni port",
                    port.name
                );
                return Err(ErrorKind::ConfigurationError(format!(
                    "Port {} : native kni and virtio ports must not define an associated kni port",
                    port.name
                ))
                .into());
            }

            debug!("initialize: {}", port);
            match PmdPort::new_port_from_configuration(port, None) {
                Ok(p) => {
                    if port.kni.is_some() {
                        kni2pci.insert(port.kni.as_ref().unwrap().clone(), p.clone());
                    }
                    update_context(p)?;
                }
                Err(e) => {
                    return Err(ErrorKind::ConfigurationError(format!(
                        "Port {} could not be initialized {:?}",
                        port.name, e
                    ))
                    .into());
                }
            }
        }

        // now we parse all other ports like kni ports, which may be associated with one of the above ports
        // we must do this in this sequence as kni ports need for initialization the port_id of the associated port

        for port in &mut configuration.ports.iter_mut().filter(|p| p.kni.is_none()) {
            let parts: Vec<_> = port.name.splitn(2, ',').collect();
            let associated_port = kni2pci.get(&parts[0][..]);
            debug!("initialize: {} - {}", port, parts[0]);
            match PmdPort::new_port_from_configuration(port, associated_port) {
                Ok(p) => update_context(p)?,
                Err(e) => match e {
                    // we ignore failed initialization of KNI ports (e.g. because of a missing associated port)
                    ErrorKind::FailedToInitializeKni(_s) => {}
                    _ => {
                        return Err(ErrorKind::ConfigurationError(format!(
                            "Port {} could not be initialized {:?}",
                            port.name, e
                        ))
                        .into())
                    }
                },
            }
        }
    }

    // as update_context is dropped, we can mutably borrow ctx again:
    for port in &mut configuration.ports {
        let parts: Vec<_> = port.name.splitn(2, ',').collect();
        if ctx.ports.contains_key(&parts[0][..]) {
            let port_instance = &ctx.ports[&parts[0][..]];
            // number of queues configured, may be larger than possible by driver, therefore correct this now
            port.rx_queues.truncate(port_instance.rxqs() as usize);
            port.tx_queues.truncate(port_instance.txqs() as usize);

            for (rx_q, core) in port.rx_queues.iter().enumerate() {
                let rx_q = rx_q as u16;
                match PmdPort::new_queue_pair(port_instance, rx_q, rx_q) {
                    Ok(q) => {
                        ctx.rx_queues
                            .entry(*core as i32)
                            .or_insert_with(|| HashSet::with_capacity(8))
                            .insert(q);
                    }
                    Err(e) => {
                        return Err(ErrorKind::ConfigurationError(format!(
                            "Queue {} on port {} could not be \
                             initialized {:?}",
                            rx_q, port.name, e
                        ))
                        .into());
                    }
                }
            }
        }
    }

    if configuration.strict {
        let other_cores: HashSet<_> = ctx.rx_queues.keys().cloned().collect();
        let core_diff: Vec<_> = other_cores.difference(&cores).map(|c| c.to_string()).collect();
        if !core_diff.is_empty() {
            let missing_str = core_diff.join(", ");
            return Err(ErrorKind::ConfigurationError(format!(
                "Strict configuration selected but core(s) {} appear \
                 in port configuration but not in cores",
                missing_str
            ))
            .into());
        }
    } else {
        cores.extend(ctx.rx_queues.keys());
    };
    ctx.active_cores = cores.into_iter().collect();
    Ok(ctx)
}
