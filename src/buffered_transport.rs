use byteorder::{self, ByteOrder};
#[cfg(unix)]
use mio::unix::UnixReady;
use mio::{PollOpt, Ready, Token};
use notify::{Notifiable, Notifies};
use scheduler::{get_event, insert_listener, remove_listener, POLL};
use std::{
	cell::RefCell, io::{self, Read, Write}, mem::swap, rc::Rc,
};
use transport::Transport;

#[derive(Clone)]
pub struct BufferedTransport {
	pub underlying:   Rc<RefCell<Transport>>,
	pub read_buffer:  Rc<RefCell<Vec<u8>>>,
	pub write_buffer: Rc<RefCell<Vec<u8>>>,
	notify_hook:      Rc<RefCell<Option<Rc<Notifiable>>>>,
	closed:           Rc<RefCell<bool>>,
	key:              Rc<RefCell<usize>>,
	pub read_limit:   Rc<RefCell<usize>>,
}

impl BufferedTransport {
	pub fn create(transport: Transport) -> BufferedTransport {
		let result = BufferedTransport {
			underlying:   Rc::new(RefCell::new(transport)),
			read_buffer:  Rc::new(RefCell::new(Vec::new())),
			write_buffer: Rc::new(RefCell::new(Vec::new())),
			notify_hook:  Rc::new(RefCell::new(None)),
			closed:       Rc::new(RefCell::new(false)),
			key:          Rc::new(RefCell::new(0)),
			read_limit:   Rc::new(RefCell::new(16384)),
		};
		let key = insert_listener(Rc::new(result.clone()));
		*result.key.borrow_mut() = key;
		result.register();
		result
	}

	#[cfg(unix)]
	pub fn create_pair() -> (BufferedTransport, BufferedTransport) {
		use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};
		let (socka, sockb) = socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::empty()).unwrap();
		(socka.into(), sockb.into())
	}

	fn register(&self) {
		POLL.with(|x| {
			x.register(
				&*self.underlying.borrow_mut(),
				Token(*self.key.borrow_mut()),
				Ready::readable(),
				PollOpt::edge(),
			).unwrap()
		});
	}

	pub fn available(&self) -> usize {
		self.read_buffer.borrow_mut().len()
	}

	pub fn take_chunk(&self, size: usize) -> Option<Vec<u8>> {
		let mut lock = self.read_buffer.borrow_mut();
		if lock.len() < size {
			trace!("No chunk of size {} available.", size);
			return None;
		}
		trace!("Taking chunk: {}", size);
		let mut tail = lock.split_off(size);
		swap(&mut *lock, &mut tail);
		Some(tail)
	}

	pub fn take(&self) -> Vec<u8> {
		let mut new = Vec::new();
		swap(&mut *self.read_buffer.borrow_mut(), &mut new);
		self.update_registration();
		new
	}

	pub fn send_message(&self, data: &[u8]) {
		if data.len() > 65535 {
			panic!("Message too long!");
		}
		let mut len = [0u8; 2];
		byteorder::BE::write_u16(&mut len, data.len() as u16);
		self.put(&len);
		self.put(&data);
	}

	pub fn recv_message(&self) -> Option<Vec<u8>> {
		if self.available() < 2 {
			return None;
		}
		let preamble = byteorder::BE::read_u16(&self.read_buffer.borrow_mut()[..2]);
		if self.available() < 2 + preamble as usize {
			return None;
		}
		self.take_chunk(2).unwrap();
		let result = Some(self.take_chunk(preamble as usize).unwrap());
		self.update_registration();
		result
	}

	pub fn recv_all_messages(&self) -> Vec<Vec<u8>> {
		let mut result = Vec::new();
		while let Some(msg) = self.recv_message() {
			result.push(msg);
		}
		result
	}

	pub fn put(&self, data: &[u8]) {
		self.write_buffer.borrow_mut().extend(data);
		self.update_registration()
	}

	pub fn has_write_space(&self) -> bool {
		self.write_buffer.borrow_mut().len() < 2048
	}

	pub fn is_closed(&self) -> bool {
		self.closed.borrow_mut().clone()
	}

	fn update_registration(&self) {
		if self.closed.borrow_mut().clone() {
			if POLL.with(|x| x.deregister(&*self.underlying.borrow_mut())).is_ok() {
				remove_listener(self.key.borrow_mut().clone());
			}
			return;
		}
		let mut readiness = Ready::empty();
		if self.write_buffer.borrow_mut().len() > 0 {
			readiness |= Ready::writable();
		}
		if self.read_buffer.borrow_mut().len() < self.read_limit.borrow_mut().clone() {
			readiness |= Ready::readable();
		}
		POLL.with(|x| {
			x.reregister(&*self.underlying.borrow_mut(), Token(*self.key.borrow_mut()), readiness, PollOpt::edge())
				.unwrap()
		});
	}
}

impl Notifiable for BufferedTransport {
	fn notify(&self) {
		let event = get_event();
		if event.readiness().is_readable() {
			let result = self.underlying.borrow_mut().read_to_end(&mut *self.read_buffer.borrow_mut());
			trace!("Read result: {:?}", result);
			trace!("Read buffer size: {:?}", self.read_buffer.borrow_mut().len());
			if let Ok(size) = result {
				if size == 0 {
					*self.closed.borrow_mut() = true;
				}
			}
		}
		if event.readiness().is_writable() {
			loop {
				if self.write_buffer.borrow_mut().len() == 0 {
					break;
				}
				let result = self.underlying.borrow_mut().write(&self.write_buffer.borrow_mut()[..]);
				trace!("Write result: {:?}", result);
				if let Err(result) = result {
					if result.kind() == io::ErrorKind::WouldBlock {
						break;
					}
				} else {
					let tail = self.write_buffer.borrow_mut().split_off(result.unwrap());
					*self.write_buffer.borrow_mut() = tail;
				}
			}
		}
		#[cfg(unix)]
		{
			if UnixReady::from(event.readiness()).is_hup() {
				*self.closed.borrow_mut() = true;
			}
		}
		if self.notify_hook.borrow_mut().is_some() {
			trace!("Notifying forward");
			let hook = self.notify_hook.borrow_mut().as_ref().unwrap().clone();
			hook.notify();
			trace!("Read buffer size after notifying forward: {}", self.read_buffer.borrow_mut().len());
		}
		self.update_registration();
	}
}

impl Notifies for BufferedTransport {
	fn set_notify(&self, receiver: Rc<Notifiable>) {
		*self.notify_hook.borrow_mut() = Some(receiver);
	}
}

impl<T: Into<Transport>> From<T> for BufferedTransport {
	fn from(transport: T) -> BufferedTransport {
		BufferedTransport::create(transport.into())
	}
}
