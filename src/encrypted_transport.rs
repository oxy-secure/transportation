use encrypt::{decrypt_frame, encrypt_frame};
use num::BigUint;
use std::{cell::RefCell, mem::swap, rc::Rc};
use BufferedTransport;
use Notifiable;
use Notifies;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EncryptionPerspective {
	Alice,
	Bob,
}

#[derive(Clone)]
pub struct EncryptedTransport {
	internal: Rc<RefCell<EncryptedTransportInternal>>,
}

struct EncryptedTransportInternal {
	bt:                BufferedTransport,
	perspective:       EncryptionPerspective,
	inbound_iv:        BigUint,
	outbound_iv:       BigUint,
	inbound_cleartext: Vec<u8>,
	inbound_messages:  Vec<Vec<u8>>,
	inbound_key:       Vec<u8>,
	outbound_key:      Vec<u8>,
	notify_hook:       Option<Rc<Notifiable>>,
}

impl EncryptedTransport {
	pub fn create<T: Into<BufferedTransport>>(transport: T, perspective: EncryptionPerspective, key_seed: &[u8]) -> EncryptedTransport {
		let transport: BufferedTransport = transport.into();
		let internal = EncryptedTransportInternal {
			bt:                transport,
			perspective:       perspective,
			inbound_iv:        BigUint::from(0u8),
			outbound_iv:       BigUint::from(0u8),
			inbound_cleartext: Vec::new(),
			inbound_messages:  Vec::new(),
			inbound_key:       Vec::new(),
			outbound_key:      Vec::new(),
			notify_hook:       None,
		};
		let result = EncryptedTransport {
			internal: Rc::new(RefCell::new(internal)),
		};
		result.rekey(key_seed);
		result.internal.borrow_mut().bt.set_notify(Rc::new(result.clone()));
		result
	}

	pub fn is_drained_forward(&self) -> bool {
		let internal = self.internal.borrow_mut();
		let write_buffer = internal.bt.write_buffer.borrow_mut();
		write_buffer.is_empty()
	}
	
	pub fn is_closed(&self) -> bool {
		self.internal.borrow_mut().bt.is_closed()
	}

	pub fn rekey(&self, seed: &[u8]) {
		use EncryptionPerspective::{Alice, Bob};
		let mut lock = self.internal.borrow_mut();
		let (akey, bkey) = EncryptedTransport::make_keys(seed);
		let (ikey, okey) = match lock.perspective {
			Alice => (akey, bkey),
			Bob => (bkey, akey),
		};
		lock.inbound_key = ikey;
		lock.outbound_key = okey;
	}

	pub fn has_write_space(&self) -> bool {
		self.internal.borrow_mut().bt.has_write_space()
	}

	pub fn make_keys(seed: &[u8]) -> (Vec<u8>, Vec<u8>) {
		use ring::{digest::SHA512, pbkdf2::derive};
		let mut alice = [0u8; 32];
		let mut bob = [0u8; 32];
		derive(&SHA512, 10240, b"alice", seed, &mut alice[..]);
		derive(&SHA512, 10240, b"bob", seed, &mut bob[..]);
		(alice.to_vec(), bob.to_vec())
	}

	fn use_iv(num: &mut BigUint) -> Vec<u8> {
		let mut result = num.to_bytes_be();
		*num += 1u8;
		if result.len() > 12 {
			panic!("Used up all the IV values!");
		}
		result.resize(12, 0);
		result
	}

	fn use_outbound_iv(&self) -> Vec<u8> {
		EncryptedTransport::use_iv(&mut self.internal.borrow_mut().outbound_iv)
	}

	fn use_inbound_iv(&self) -> Vec<u8> {
		EncryptedTransport::use_iv(&mut self.internal.borrow_mut().inbound_iv)
	}

	pub fn send_chunk(&self, chunk: &[u8]) {
		let chunk_size = chunk.len();
		assert!(chunk_size <= 255);
		let mut frame = Vec::with_capacity(256 + 16);
		frame.push(chunk_size as u8);
		frame.extend(chunk);
		frame.resize(256, 0);
		let iv = self.use_outbound_iv();
		encrypt_frame(&mut frame, &self.internal.borrow_mut().outbound_key[..], &iv[..]);
		self.internal.borrow_mut().bt.put(&frame[..]);
	}

	pub fn send(&self, message: &[u8]) {
		let mut chunk_size = 255;
		for i in message.chunks(255) {
			self.send_chunk(i);
			chunk_size = i.len();
		}
		if chunk_size == 255 {
			self.send_chunk(b"");
		}
	}

	pub fn recv(&self) -> Option<Vec<u8>> {
		self.decrypt_available();
		let mut lock = self.internal.borrow_mut();
		if lock.inbound_messages.len() == 0 {
			return None;
		}
		Some(lock.inbound_messages.remove(0))
	}

	pub fn recv_all(&self) -> Vec<Vec<u8>> {
		self.decrypt_available();
		let mut new = Vec::new();
		swap(&mut new, &mut self.internal.borrow_mut().inbound_messages);
		new
	}

	fn decrypt_chunk(&self, chunk: &mut [u8]) {
		let iv = self.use_inbound_iv();
		let usable = decrypt_frame(chunk, &self.internal.borrow_mut().inbound_key[..], &iv[..]);
		assert!(usable == 256);
		let text_len = chunk[0] as usize;
		self.internal.borrow_mut().inbound_cleartext.extend(&chunk[1..(1 + text_len)]);
		if text_len < 255 {
			trace!("Unsaturated chunk detected; cutting a message");
			let mut new = Vec::new();
			swap(&mut self.internal.borrow_mut().inbound_cleartext, &mut new);
			self.internal.borrow_mut().inbound_messages.push(new);
		}
	}

	fn notify_forward(&self) {
		let hook = self.internal.borrow_mut().notify_hook.clone();
		if let Some(ref x) = hook {
			x.notify();
		};
	}

	fn decrypt_available(&self) {
		let mut amount_to_take = self.internal.borrow_mut().bt.available();
		amount_to_take -= amount_to_take % (256 + 16);
		if amount_to_take == 0 {
			return;
		}
		let mut data = self.internal.borrow_mut().bt.take_chunk(amount_to_take).unwrap();
		for chunk in data.chunks_mut(256 + 16) {
			trace!("Processing a chunk");
			self.decrypt_chunk(chunk);
		}
	}
}

impl Notifies for EncryptedTransport {
	fn set_notify(&self, other: Rc<Notifiable>) {
		self.internal.borrow_mut().notify_hook = Some(other);
	}
}

impl Notifiable for EncryptedTransport {
	fn notify(&self) {
		self.notify_forward();
	}
}
