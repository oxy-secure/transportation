//! A wrapper around mio::Poll that helps with sharing time in a single thread. The core of the library is the run() function. run() never returns
//! and the thread spends the rest of its life collecting events from an MIO poll and dispatching them.
//!
//! To use the high-level interface:
//! 1. Create a BufferedTransport from whatever IO object you want to do IO with
//! 2. Create a Notifiable and register it with your BufferedTransport using set_notify()
//! 3. Call transportation::run()
//! 4. Your notifiable will get notified when there's data. (Your notifiable should keep its own copy of the BufferedTransport so it can get at the
//! data)
//!
//! To use the low-level interface:
//! 1. Create a type that implements Notifiable ( An Fn() -> () closure is great for this )
//! 2. Register an instance of your type with the reactor using insert_listener()
//! 3. Use the usize returned by insert_listener() as an Mio::token() to register with the mio::Poll from borrow_poll(). Get your types from the
//! re-exported mio crate to ensure the versions match.
//! 4. When mio generates events for your token, transportation will call your instance's
//! notify() function. Get a copy of the current event by calling get_event()

pub extern crate mio;
#[cfg(unix)]
extern crate nix;
#[macro_use]
extern crate log;
extern crate byteorder;

mod buffered_transport;
#[cfg(unix)]
mod fd_adapter;
mod notify;
mod scheduler;
#[cfg(unix)]
mod signals;
mod transport;

pub use buffered_transport::BufferedTransport;
pub use notify::{Notifiable, Notifies};
pub use scheduler::{borrow_poll, flush, get_event, insert_listener, remove_listener, run, set_timeout};
#[cfg(unix)]
pub use signals::get_signal_name;
#[cfg(unix)]
pub use signals::set_signal_handler;
pub use transport::Transport;
