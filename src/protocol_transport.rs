use serde::{de::DeserializeOwned, Serialize};
use serde_cbor;
use std::{
	io::{Read, Write},
	rc::Rc,
};
use MessageTransport;
use Notifiable;
use Notifies;

pub struct ProtocolTransport {
	pub mt:                   MessageTransport,
	pub outbound_compression: bool,
	pub inbound_compression:  bool,
}

fn compress(data: &[u8]) -> Vec<u8> {
	let compressed_data = Vec::with_capacity(data.len());
	let mut encoder = ::libflate::zlib::Encoder::new(compressed_data).expect("Failed to create outbound compression encoder");
	encoder.write_all(&data[..]).expect("Failed to compress outbound message");
	encoder.finish().into_result().expect("Failed to compress outbound message.")
}

fn decompress(data: &[u8]) -> Vec<u8> {
	let mut decoder = ::libflate::zlib::Decoder::new(data).expect("Failed to create inbound compression decoder");
	let mut buf = Vec::new();
	decoder.read_to_end(&mut buf).expect("Failed to decompress inbound message");
	buf
}

impl ProtocolTransport {
	pub fn create<T: Into<MessageTransport>>(transport: T) -> ProtocolTransport {
		ProtocolTransport {
			mt:                   transport.into(),
			inbound_compression:  false,
			outbound_compression: false,
		}
	}

	pub fn send<T: Serialize>(&self, message: T) {
		let mut data = serde_cbor::ser::to_vec_packed(&message).unwrap();
		if self.outbound_compression {
			data = compress(&data);
		}
		self.mt.send(&data[..]);
	}

	pub fn recv<T: DeserializeOwned>(&self) -> Option<T> {
		let message = self.mt.recv();
		if message.is_none() {
			return None;
		}
		let mut message = message.unwrap();
		if self.inbound_compression {
			message = decompress(&message);
		}
		trace!("Trying to deserialize input: {:?}", message);
		serde_cbor::from_slice(&message[..]).expect("Failed to deserialize protocol message")
	}

	pub fn recv_tolerant<T: DeserializeOwned>(&self) -> Option<Option<T>> {
		let message = self.mt.recv();
		if message.is_none() {
			return None;
		}
		let mut message = message.unwrap();
		if self.inbound_compression {
			message = decompress(&message);
		}
		trace!("Trying to deserialize input: {:?}", message);
		Some(serde_cbor::from_slice(&message[..]).ok())
	}

	pub fn recv_all<T: DeserializeOwned>(&self) -> Vec<T> {
		let mut result = Vec::new();
		for mut i in self.mt.recv_all() {
			if self.inbound_compression {
				i = decompress(&i);
			}
			result.push(serde_cbor::from_slice(&i).expect("Failed to deserialize protocol message."));
		}
		result
	}

	pub fn recv_all_tolerant<T: DeserializeOwned>(&self) -> Vec<Option<T>> {
		let mut result = Vec::new();
		for mut i in self.mt.recv_all() {
			if self.inbound_compression {
				i = decompress(&i);
			}
			result.push(serde_cbor::from_slice(&i).ok());
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
