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
//! 1. Create a type that implements Notifiable
//! 2. Register an instance of your type with the reactor using insert_listener()
//! 3. Use the usize returned by insert_listener() as an Mio::token() to register with the mio::Poll from borrow_poll(). Get your types from the
//! re-exported mio crate to ensure the versions match.
//! 4. When mio generates events for your token, transportation will call your instance's
//! notify() function. Get a copy of the current event by calling get_event()

pub extern crate mio;
#[cfg(unix)]
extern crate nix;
#[cfg(feature = "encrypt")]
extern crate num;
#[cfg(feature = "encrypt")]
pub extern crate ring;
#[cfg(feature = "protocol")]
extern crate serde;
#[cfg(feature = "protocol")]
extern crate serde_cbor;
#[cfg(feature = "encrypt")]
pub extern crate untrusted; // Seems bizzare that ring itself does not reexport untrusted.
#[macro_use]
extern crate log;
#[macro_use]
#[cfg(feature = "encrypt")]
extern crate lazy_static;
extern crate byteorder;

mod buffered_transport;
#[cfg(feature = "encrypt")]
mod encrypt;
#[cfg(feature = "encrypt")]
mod encrypted_transport;
#[cfg(unix)]
mod fd_adapter;
mod message_transport;
mod notify;
#[cfg(feature = "protocol")]
mod protocol_transport;
mod scheduler;
#[cfg(unix)]
mod signals;
mod transport;

pub use buffered_transport::BufferedTransport;
#[cfg(feature = "encrypt")]
pub use encrypted_transport::EncryptedTransport;
#[cfg(feature = "encrypt")]
pub use encrypted_transport::EncryptionPerspective;
pub use message_transport::MessageTransport;
pub use notify::{Notifiable, Notifies};
#[cfg(feature = "protocol")]
pub use protocol_transport::ProtocolTransport;
pub use scheduler::{borrow_poll, get_event, insert_listener, remove_listener, run};
#[cfg(unix)]
pub use signals::get_signal_name;
#[cfg(unix)]
pub use signals::set_signal_handler;

#[cfg(feature = "encrypt")]
lazy_static! {
	pub static ref RNG: ring::rand::SystemRandom = ring::rand::SystemRandom::new();
}
