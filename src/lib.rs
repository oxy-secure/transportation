#![deny(missing_docs)]

//! This crate provides a thin callback wrapper around MIO. For each thread, a thread_local [`mio::Poll`] is maintained. This poll can be accessed
//! with [`borrow_poll`]. To listen for events on an [`Evented`](mio::Evented), do the following:
//! ```
//! use transportation::mio;
//!
//! let token = transportation::insert_listener(|event| println!("Got event: {:?}"));
//! transportation::borrow_poll(|poll| poll.register(&evented, mio::Token(token), mio::Ready::readable(), mio::PollOpt::level()));
//! transportation::run();
//! ```

pub extern crate mio;
#[macro_use]
extern crate lazy_static;

mod scheduler;

pub use scheduler::{
	borrow_poll, clear_interval, clear_timeout, insert_listener, remove_listener, run, run_in_thread, run_worker, set_interval, set_timeout, stop,
};
