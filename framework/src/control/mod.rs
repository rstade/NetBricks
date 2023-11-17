#[cfg(target_os = "linux")]
pub use self::epoll::*;
use std::os::fd::AsFd;

#[cfg(target_os = "linux")]
#[path = "linux/epoll.rs"]
mod epoll;
#[cfg(feature = "sctp")]
pub mod sctp;
pub mod tcp;

pub type Available = u64;

pub const NONE: u64 = 0x0;
pub const READ: u64 = 0x1;
pub const WRITE: u64 = 0x2;
pub const HUP: u64 = 0x4;

pub struct IOScheduler<'fd, Fd: AsFd> {
    fd: Fd,
    scheduler: &'fd PollHandle,
    token: Token,
}

impl<'fd, Fd: AsFd> IOScheduler<'fd, Fd> {
    pub fn new(scheduler: &'fd PollHandle, fd: Fd, token: Token) -> IOScheduler<Fd> {
        scheduler.new_io_fd(fd.as_fd(), token);
        IOScheduler { fd, scheduler, token }
    }

    pub fn schedule_read(&self) {
        self.scheduler.schedule_read(&self.fd, self.token);
    }

    pub fn schedule_write(&self) {
        self.scheduler.schedule_write(&self.fd, self.token);
    }
}
