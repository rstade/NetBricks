#![recursion_limit = "1024"]
#![deny(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_doc_comments)]
#![cfg_attr(feature = "dev", allow(unstable_features))]
// Need this since PMD port construction triggers too many arguments.
#![cfg_attr(feature = "dev", allow(too_many_arguments))]
#![cfg_attr(feature = "dev", feature(plugin))]
#![cfg_attr(feature = "dev", deny(warnings))]

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

#[cfg(feature = "sctp")]
extern crate sctp;

// For cache aware allocation
// extern crate alloc;

// Better error handling.
//#[macro_use]
//extern crate error_chain;

// Logging
#[macro_use]
extern crate log;
#[allow(dead_code)]
#[cfg(unix)]
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
