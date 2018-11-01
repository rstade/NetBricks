use super::{Executable, Scheduler};
use std::collections::HashMap;
use std::sync::mpsc::{ Receiver, RecvError, Sender, SyncSender };
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use utils;
use uuid::Uuid;
use separator::Separatable;

/// Used to keep stats about each pipeline and eventually grant tokens, etc.
pub struct Runnable {
    pub task: Box<Executable>,
    pub uuid: Uuid,
    pub name: String,
    pub cycles: u64,    // cycles used while doing some work (i.e. increasing 'count' metric)
    pub count: u64,     // packets processed (or some comparable metric for the work done)
    pub last_run: u64,
    pub is_ready: Arc<AtomicBool>,
}

impl Runnable {
    pub fn from_task<T: Executable + 'static>(uuid: Uuid, name: String, task: T) -> Runnable {
        Runnable {
            task: box task,
            uuid,
            name,
            cycles: 0,
            count: 0,
            last_run: utils::rdtsc_unsafe(),
            is_ready: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn from_boxed_task(uuid: Uuid, name: String, task: Box<Executable>) -> Runnable {
        Runnable {
            task,
            uuid,
            name,
            cycles: 0,
            count: 0,
            last_run: utils::rdtsc_unsafe(),
            is_ready: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline]
    pub fn ready(&self) -> &Self {
        self.is_ready.store( true, Ordering::SeqCst);
        self
    }

    #[inline]
    pub fn unready(&self) -> &Self {
        self.is_ready.store( false, Ordering::SeqCst);
        self
    }

    #[inline]
    pub fn move_ready(self) -> Self {
        self.is_ready.store( true, Ordering::SeqCst);
        self
    }

    #[inline]
    pub fn move_unready(self) -> Self {
        self.is_ready.store( false, Ordering::SeqCst);
        self
    }

    #[inline]
    pub fn is_ready(&self) -> bool {
        self.is_ready.load(Ordering::SeqCst)
    }

    #[inline]
    fn get_ready_atomic(&self) -> Arc<AtomicBool> {
        self.is_ready.clone()
    }
}

/// A very simple round-robin scheduler. This should really be more of a DRR scheduler.
pub struct StandaloneScheduler {
    /// The set of runnable items. Note we currently don't have a blocked queue.
    run_q: Vec<Runnable>,
    /// A map from uuid of runnable item to index of runnable item in run_q
    uuid2index: HashMap<Uuid, usize>,
    /// Next task to run.
    next_task: usize,
    /// Channel to communicate and synchronize with scheduler.
    sched_channel: Receiver<SchedulerCommand>,
    /// Reply channel e.g. for sending performance data
    sender: Sender<SchedulerReply>,
    /// core id
    core: i32,
    /// Signal scheduler should continue executing tasks.
    execute_loop: bool,
    /// Signal scheduler should shutdown.
    shutdown: bool,
}

/// Messages that can be sent on the scheduler channel to add or remove tasks.
pub enum SchedulerCommand {
    Add((Uuid, String, Box<Executable + Send>)),
    Run(Box<Fn(&mut StandaloneScheduler) + Send>),
    SetTaskState(Uuid, bool),
    SetTaskStateAll(bool),
    Execute,
    Shutdown,
    Handshake(SyncSender<bool>),
    GetPerformance,
}

pub enum SchedulerReply {
    PerformanceData(i32, HashMap<Uuid, (String, u64, u64)>), //core id, uuid of task, task name, consumed cycles, count
}

const DEFAULT_Q_SIZE: usize = 256;

/*
impl Default for StandaloneScheduler {
    fn default() -> StandaloneScheduler {
        StandaloneScheduler::new()
    }
}
*/

impl Scheduler for StandaloneScheduler {
    /// Add a task to the current scheduler. The  caller must assign a uuid to the task.
    fn add_runnable(&mut self, runnable: Runnable) -> usize {
        let index = self.run_q.len();
        self.uuid2index.insert(runnable.uuid, index);
        self.run_q.push(runnable);
        index
    }
}

impl StandaloneScheduler {
    pub fn new_with_channel(
        core: i32,
        receiver: Receiver<SchedulerCommand>,
        sender: Sender<SchedulerReply>,
    ) -> StandaloneScheduler {
        StandaloneScheduler::new_with_channel_and_capacity(core, receiver, sender, DEFAULT_Q_SIZE)
    }

    pub fn new_with_channel_and_capacity<'b>(
        core: i32,
        receiver: Receiver<SchedulerCommand>,
        sender: Sender<SchedulerReply>,
        capacity: usize,
    ) -> StandaloneScheduler {
        StandaloneScheduler {
            run_q: Vec::with_capacity(capacity),
            uuid2index: HashMap::with_capacity(capacity),
            next_task: 0,
            sched_channel: receiver,
            sender,
            core,
            execute_loop: false,
            shutdown: true,
        }
    }

    #[inline]
    pub fn set_task_state(&mut self, uuid: &Uuid, ready: bool) -> Option<bool> {
        match self.uuid2index.get(uuid) {
            Some(index) => {
                let previous=self.run_q[*index].is_ready.swap(ready, Ordering::SeqCst);
                Some(previous)
            }
            None => { None }
        }
    }

    pub fn get_ready_flag(&self, uuid: &Uuid) -> Option<Arc<AtomicBool>> {
        match self.uuid2index.get(uuid) {
            Some(index) => {
                Some(self.run_q[*index].get_ready_atomic())
            }
            None => { None }
        }
    }

    fn handle_request(&mut self, request: SchedulerCommand) {
        match request {
            SchedulerCommand::Add((uuid, name, ex)) => {
                self.uuid2index.insert(uuid, self.run_q.len());
                self.run_q.push(Runnable::from_boxed_task(uuid, name, ex));
            }
            SchedulerCommand::Run(f) => f(self),
            SchedulerCommand::Execute => self.execute_loop(),
            SchedulerCommand::Shutdown => {
                self.execute_loop = false;
                self.shutdown = true;
            }
            SchedulerCommand::SetTaskState(uuid, state) => {
                self.set_task_state(&uuid, state);
            }
            SchedulerCommand::SetTaskStateAll(state) => {
                for r in &mut self.run_q {
                    r.ready();
                }
                debug!("core {}: set task state all {:?} at {:>20}", self.core, state, utils::rdtsc_unsafe().separated_string());
            }
            SchedulerCommand::GetPerformance => {
                let mut data:  HashMap<Uuid, (String, u64, u64)> = HashMap::with_capacity(DEFAULT_Q_SIZE);
                for r in &self.run_q {
                    data.insert(r.uuid, (r.name.clone(), r.cycles, r.count));
                }
                self.sender.send(SchedulerReply::PerformanceData(self.core, data)).unwrap();
            }
            SchedulerCommand::Handshake(chan) => {
                chan.send(true).unwrap(); // Inform context about reaching barrier.
                thread::park();
            }
        }
    }

    pub fn handle_requests(&mut self) {
        self.shutdown = false;
        // Note this rather bizarre structure here to get shutting down hooked in.
        while let Ok(cmd) = {
            if self.shutdown {
                Err(RecvError)
            } else {
                self.sched_channel.recv()
            }
        } {
            self.handle_request(cmd)
        }
        info!(
            "Scheduler exiting {}",
            thread::current().name().unwrap_or_else(|| "unknown-name")
        );
    }

    #[inline]
    fn execute_internal(&mut self, begin: u64) -> u64 {
        let time = {
            let task = &mut (&mut self.run_q[self.next_task]);
            if task.is_ready() {
                let count = task.task.execute();
                let end = utils::rdtsc_unsafe();
                if count > 0 {
                    task.count += count as u64;
                    task.cycles += end - begin;
                }
                task.last_run = end;
                end
            } else {
                utils::rdtsc_unsafe()
            }
        };

        let len = self.run_q.len();
        let next = self.next_task + 1;
        if next == len {
            self.next_task = 0;
            if let Ok(cmd) = self.sched_channel.try_recv() {
                self.handle_request(cmd);
            }
        } else {
            self.next_task = next;
        };
        time
    }

    /// Run the scheduling loop.
    pub fn execute_loop(&mut self) {
        self.execute_loop = true;
        if !self.run_q.is_empty() {
            while self.execute_loop {
                self.execute_internal(utils::rdtsc_unsafe());
            }
        }
    }

    pub fn execute_one(&mut self) {
        if !self.run_q.is_empty() {
            self.execute_internal(utils::rdtsc_unsafe());
        }
    }
}
