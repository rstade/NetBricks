#![recursion_limit = "1024"]
#![feature(ptr_internals)]
#![allow(unused_doc_comments)]
#![cfg_attr(feature = "dev", allow(unstable_features))]
// Need this since PMD port construction triggers too many arguments.
#![cfg_attr(feature = "dev", allow(too_many_arguments))]
#![cfg_attr(feature = "dev", feature(plugin))]
#![cfg_attr(feature = "dev", deny(warnings))]
extern crate byteorder;
extern crate fnv;
extern crate ipnet;
extern crate libc;
extern crate net2;
extern crate regex;
extern crate separator;
extern crate twox_hash;

#[macro_use]
extern crate serde_derive;
extern crate serde;
#[macro_use]
extern crate lazy_static;

#[cfg(feature = "sctp")]
extern crate sctp;
// TOML for scheduling configuration
extern crate toml;
// UUID for SHM naming
extern crate uuid;

// For cache aware allocation
// extern crate alloc;

// Better error handling.
//#[macro_use]
//extern crate error_chain;

// Logging
#[macro_use]
extern crate log;

extern crate core;
#[allow(dead_code)]
extern crate eui48;
#[cfg(unix)]
extern crate nix;

pub mod allocators;
pub mod common;
pub mod config;
pub mod control;
pub mod headers;
pub mod interface;
pub mod native;
pub mod operators;
pub mod queues;
pub mod scheduler;
pub mod shared_state;
pub mod state;
pub mod utils;
