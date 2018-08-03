#![feature(rust_2018_preview)]

pub extern crate mio;

mod scheduler;

pub use crate::scheduler::{get_poll, insert_callback, run, run_auto, set_interval, set_timeout};
