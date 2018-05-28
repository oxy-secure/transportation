use notify::{Notifiable, Notifies};
use std::rc::Rc;
use BufferedTransport;
#[cfg(feature = "encrypt")]
use EncryptedTransport;

pub enum MessageTransport {
	#[cfg(feature = "encrypt")]
	EncryptedTransport(EncryptedTransport),
	BufferedTransport(BufferedTransport),
}

impl MessageTransport {
	pub fn send(&self, message: &[u8]) {
		match self {
			#[cfg(feature = "encrypt")]
			MessageTransport::EncryptedTransport(et) => et.send(message),
			MessageTransport::BufferedTransport(bt) => bt.send_message(message),
		}
	}

	pub fn recv(&self) -> Option<Vec<u8>> {
		match self {
			#[cfg(feature = "encrypt")]
			MessageTransport::EncryptedTransport(et) => et.recv(),
			MessageTransport::BufferedTransport(bt) => bt.recv_message(),
		}
	}

	pub fn recv_all(&self) -> Vec<Vec<u8>> {
		match self {
			#[cfg(feature = "encrypt")]
			MessageTransport::EncryptedTransport(et) => et.recv_all(),
			MessageTransport::BufferedTransport(bt) => bt.recv_all_messages(),
		}
	}

	pub fn has_write_space(&self) -> bool {
		match self {
			#[cfg(feature = "encrypt")]
			MessageTransport::EncryptedTransport(et) => et.has_write_space(),
			MessageTransport::BufferedTransport(bt) => bt.has_write_space(),
		}
	}

	pub fn is_closed(&self) -> bool {
		match self {
			#[cfg(feature = "encrypt")]
			MessageTransport::EncryptedTransport(et) => et.is_closed(),
			MessageTransport::BufferedTransport(bt) => bt.is_closed(),
		}
	}
}

impl Notifiable for MessageTransport {
	fn notify(&self) {
		match self {
			#[cfg(feature = "encrypt")]
			MessageTransport::EncryptedTransport(et) => et.notify(),
			MessageTransport::BufferedTransport(bt) => bt.notify(),
		}
	}
}

impl Notifies for MessageTransport {
	fn set_notify(&self, callback: Rc<Notifiable>) {
		match self {
			#[cfg(feature = "encrypt")]
			MessageTransport::EncryptedTransport(et) => et.set_notify(callback),
			MessageTransport::BufferedTransport(bt) => bt.set_notify(callback),
		}
	}
}

#[cfg(feature = "encrypt")]
impl From<EncryptedTransport> for MessageTransport {
	fn from(et: EncryptedTransport) -> MessageTransport {
		MessageTransport::EncryptedTransport(et)
	}
}

impl From<BufferedTransport> for MessageTransport {
	fn from(bt: BufferedTransport) -> MessageTransport {
		MessageTransport::BufferedTransport(bt)
	}
}
