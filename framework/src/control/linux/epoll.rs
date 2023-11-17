use super::{Available, HUP, NONE, READ, WRITE};

use nix::sys::epoll::*;
use std::default::Default;
use std::os::fd::AsFd;
use std::slice;

pub type Token = u64;

pub struct PollHandle {
    epoll: Epoll,
}

impl PollHandle {
    // pub fn schedule_read<Fd: AsFd>(&self, file: &Fd, token: Token) {
    //    self.schedule_read_rawfd(file, token).expect("schedule_read failed");
    // }

    pub fn schedule_read<Fd: AsFd>(&self, fd: &Fd, token: Token) {
        let event = EpollEvent::new(
            EpollFlags::EPOLLIN | EpollFlags::EPOLLET | EpollFlags::EPOLLONESHOT,
            token,
        );
        // epoll_ctl(self.epoll_fd, EpollOp::EpollCtlMod, fd, &mut event).unwrap();
        self.epoll.add(fd, event).expect("Epoll::add failed");
    }

    //pub fn schedule_write<Fd: AsRawFd>(&self, file: &Fd, token: Token) {
    //    self.schedule_write_rawfd(file.as_raw_fd(), token);
    //}

    pub fn schedule_write<Fd: AsFd>(&self, file: &Fd, token: Token) {
        let mut event = EpollEvent::new(
            EpollFlags::EPOLLOUT | EpollFlags::EPOLLET | EpollFlags::EPOLLONESHOT,
            token,
        );
        //epoll_ctl(self.epoll_fd, EpollOp::EpollCtlMod, fd, &mut event).unwrap();
        self.epoll.modify(file, &mut event).expect("Epoll.modify failed");
    }

    /// This assumes file is already set to be non-blocking. This must also be called only the first time round.
    pub fn new_io_port<Fd: AsFd>(&self, file: &Fd, token: Token) {
        self.new_io_fd(file, token);
    }

    pub fn new_io_fd<Fd: AsFd>(&self, fd: Fd, token: Token) {
        let event = EpollEvent::new(EpollFlags::EPOLLET | EpollFlags::EPOLLONESHOT, token);
        //epoll_ctl(self.epoll_fd, EpollOp::EpollCtlAdd, fd, &mut event).unwrap();
        self.epoll.add(fd, event).expect("Epoll.add failed");
    }
}

pub struct PollScheduler {
    poll_handle: PollHandle,
    ready_tokens: Vec<EpollEvent>,
    events: usize,
}

impl Default for PollScheduler {
    fn default() -> PollScheduler {
        PollScheduler::new()
    }
}

impl PollScheduler {
    pub fn poll_handle(&self) -> &PollHandle {
        &self.poll_handle
    }

    pub fn new() -> PollScheduler {
        PollScheduler {
            poll_handle: PollHandle {
                epoll: Epoll::new(EpollCreateFlags::empty()).expect("Epoll::new failed"),
            }, // epoll_create().unwrap(),
            ready_tokens: Vec::with_capacity(32),
            events: 0,
        }
    }

    #[inline]
    fn epoll_kind_to_available(&self, kind: &EpollFlags) -> Available {
        let mut available = NONE;
        if kind.contains(EpollFlags::EPOLLIN) {
            available |= READ
        };
        if kind.contains(EpollFlags::EPOLLOUT) {
            available |= WRITE
        };
        if kind.contains(EpollFlags::EPOLLHUP) || kind.contains(EpollFlags::EPOLLERR) {
            available |= HUP
        };
        available
    }

    pub fn get_token_noblock(&mut self) -> Option<(Token, Available)> {
        if self.events > 0 {
            self.events -= 1;
            self.ready_tokens.pop()
        } else {
            let dest =
                unsafe { slice::from_raw_parts_mut(self.ready_tokens.as_mut_ptr(), self.ready_tokens.capacity()) };
            // self.events = epoll_wait(self.epoll_fd, dest, 0).unwrap();
            self.events = self.poll_handle.epoll.wait(dest, 0).expect("Epoll::wait failed");
            unsafe { self.ready_tokens.set_len(self.events) };
            self.ready_tokens.pop()
        }
        .map(|t| (t.data(), self.epoll_kind_to_available(&t.events())))
    }
}
