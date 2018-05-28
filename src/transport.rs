#[cfg(unix)]
use fd_adapter::FdAdapter;
use mio::{net::TcpStream, Evented, Poll, PollOpt, Ready, Token};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::io::RawFd;

pub enum Transport {
	TcpStream(TcpStream),
	#[cfg(unix)]
	FdAdapter(FdAdapter),
}

impl Read for Transport {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		match *self {
			#[cfg(unix)]
			Transport::FdAdapter(ref mut x) => x.read(buf),
			Transport::TcpStream(ref mut x) => x.read(buf),
		}
	}
}

impl Write for Transport {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		match *self {
			#[cfg(unix)]
			Transport::FdAdapter(ref mut x) => x.write(buf),
			Transport::TcpStream(ref mut x) => x.write(buf),
		}
	}

	fn flush(&mut self) -> io::Result<()> {
		match *self {
			#[cfg(unix)]
			Transport::FdAdapter(ref mut x) => x.flush(),
			Transport::TcpStream(ref mut x) => x.flush(),
		}
	}
}

impl Evented for Transport {
	fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
		match *self {
			#[cfg(unix)]
			Transport::FdAdapter(ref x) => x.register(poll, token, interest, opts),
			Transport::TcpStream(ref x) => x.register(poll, token, interest, opts),
		}
	}

	fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
		match *self {
			#[cfg(unix)]
			Transport::FdAdapter(ref x) => x.reregister(poll, token, interest, opts),
			Transport::TcpStream(ref x) => x.reregister(poll, token, interest, opts),
		}
	}

	fn deregister(&self, poll: &Poll) -> io::Result<()> {
		match *self {
			#[cfg(unix)]
			Transport::FdAdapter(ref x) => x.deregister(poll),
			Transport::TcpStream(ref x) => x.deregister(poll),
		}
	}
}

#[cfg(unix)]
impl From<RawFd> for Transport {
	fn from(fd: RawFd) -> Transport {
		Transport::FdAdapter(FdAdapter::from(fd))
	}
}

impl From<TcpStream> for Transport {
	fn from(stream: TcpStream) -> Transport {
		Transport::TcpStream(stream)
	}
}

impl From<::std::net::TcpStream> for Transport {
	fn from(stream: ::std::net::TcpStream) -> Transport {
		let stream = ::mio::net::TcpStream::from_stream(stream).unwrap();
		From::from(stream)
	}
}
