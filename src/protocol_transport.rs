use serde::{de::DeserializeOwned, Serialize};
use serde_cbor;
use std::rc::Rc;
use MessageTransport;
use Notifiable;
use Notifies;

pub struct ProtocolTransport {
	pub mt: MessageTransport,
}

impl ProtocolTransport {
	pub fn create<T: Into<MessageTransport>>(transport: T) -> ProtocolTransport {
		ProtocolTransport { mt: transport.into() }
	}

	pub fn send<T: Serialize>(&self, message: T) {
		let data = serde_cbor::ser::to_vec_packed(&message).unwrap();
		self.mt.send(&data[..]);
	}

	pub fn recv<T: DeserializeOwned>(&self) -> Option<T> {
		let message = self.mt.recv();
		if message.is_none() {
			return None;
		}
		let message = message.unwrap();
		trace!("Trying to deserialize input: {:?}", message);
		serde_cbor::from_slice(&message[..]).unwrap()
	}

	pub fn recv_all<T: DeserializeOwned>(&self) -> Vec<T> {
		let mut result = Vec::new();
		for i in self.mt.recv_all() {
			result.push(serde_cbor::from_slice(&i).unwrap());
		}
		result
	}

	pub fn has_write_space(&self) -> bool {
		self.mt.has_write_space()
	}

	pub fn is_closed(&self) -> bool {
		self.mt.is_closed()
	}
}

impl Notifiable for ProtocolTransport {
	fn notify(&self) {
		self.mt.notify()
	}
}

impl Notifies for ProtocolTransport {
	fn set_notify(&self, other: Rc<Notifiable>) {
		self.mt.set_notify(other)
	}
}
