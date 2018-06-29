use mio::{unix::EventedFd, Evented, Poll, PollOpt, Ready, Token};
use nix::{
	self,
	fcntl::{
		fcntl,
		FcntlArg::{F_GETFL, F_SETFL},
		OFlag,
	},
};
use std::{
	io::{self, Read, Result, Write},
	os::unix::io::RawFd,
};

pub struct FdAdapter {
	pub fd: RawFd,
}

fn errno<T>() -> io::Result<T> {
	Err(io::Error::last_os_error())
}

impl FdAdapter {
	pub fn from(fd: RawFd) -> FdAdapter {
		let bits = fcntl(fd, F_GETFL);
		if let Ok(bits) = bits {
			let mut flags = OFlag::from_bits_truncate(bits);
			flags |= OFlag::O_NONBLOCK;
			fcntl(fd, F_SETFL(flags)).unwrap();
		}
		FdAdapter { fd: fd }
	}
}

impl Read for FdAdapter {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
		nix::unistd::read(self.fd, buf).or(errno())
	}
}

impl Write for FdAdapter {
	fn write(&mut self, buf: &[u8]) -> Result<usize> {
		nix::unistd::write(self.fd, buf).or(errno())
	}

	fn flush(&mut self) -> Result<()> {
		nix::unistd::fsync(self.fd).or(errno())
	}
}

impl Evented for FdAdapter {
	fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
		EventedFd(&self.fd).register(poll, token, interest, opts)
	}

	fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
		EventedFd(&self.fd).reregister(poll, token, interest, opts)
	}

	fn deregister(&self, poll: &Poll) -> io::Result<()> {
		EventedFd(&self.fd).deregister(poll)
	}
}
