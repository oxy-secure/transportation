use mio::{unix::EventedFd, Evented, Poll, PollOpt, Ready, Token};
use nix;
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
