/// All projects involve building a thread pool. This is the task equivalent for the threadpool in `NetBricks`.
/// Anything that implements Runnable can be polled by the scheduler. This thing can be a `Batch` (e.g., `SendBatch`) or
/// something else (e.g., the `GroupBy` operator). Eventually this trait will have more stuff.
pub use self::context::*;
pub use self::standalone_scheduler::*;

mod standalone_scheduler;

mod context;

pub trait Executable {
    fn execute(&mut self) -> (u32, i32); // returns #packets processed, or a comparable metric
}

impl<F> Executable for F
where
    F: FnMut() -> (u32, i32),
{
    fn execute(&mut self) -> (u32, i32) {
        (*self)()
    }
}

pub trait Scheduler {
    fn add_runnable(&mut self, runnable: Runnable) -> usize
    where
        Self: Sized;
}

pub trait Message {}
