use mio::{unix::EventedFd, PollOpt, Ready, Token};
use nix::{
	self, sys::{
		signal::Signal::{self, *},
	},
};
#[cfg(any(target_os = "linux", target_os = "android"))]
use nix::sys::signalfd::{siginfo, SfdFlags, SIGNALFD_SIGINFO_SIZE};
#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd",
		  target_os = "dragonfly", target_os = "bitrig", target_os = "macos", target_os = "ios"))]
use nix::sys::event::{KEvent, EventFilter, EventFlag, FilterFlag};
use std::{cell::RefCell, os::unix::io::RawFd, rc::Rc};
use Notifiable;

thread_local! {
	static HANDLER: RefCell<Option<Rc<Notifiable>>> = RefCell::new(None);

	#[cfg(any(target_os = "linux", target_os = "android"))]
	static SIGNALFD: RefCell<Option<RawFd>> = RefCell::new(None);
	#[cfg(any(target_os = "linux", target_os = "android"))]
	static SIGINFO: RefCell<Option<siginfo>> = RefCell::new(None);

	#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd",
	          target_os = "dragonfly", target_os = "bitrig", target_os = "macos", target_os = "ios"))]
	static KQUEUE: RefCell<Option<RawFd>> = RefCell::new(None);
	#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd",
	          target_os = "dragonfly", target_os = "bitrig", target_os = "macos", target_os = "ios"))]
	static SIGNUM: RefCell<Option<usize>> = RefCell::new(None);
}

fn mask_signals() -> nix::sys::signal::SigSet {
	let mut mask = nix::sys::signal::SigSet::empty();
	mask.add(SIGWINCH);
	mask.add(SIGCHLD);
	mask.thread_block().unwrap();
	mask
}

fn register_proxy(fd: RawFd) {
	let proxy = SignalNotificationProxy {};
	let token = ::insert_listener(Rc::new(proxy));
	::borrow_poll(|x| {
		x.register(&EventedFd(&fd), Token(token), Ready::readable(), PollOpt::level()).unwrap();
	});
}

pub fn set_signal_handler(handler: Rc<Notifiable>) {
	#[cfg(any(target_os = "linux", target_os = "android"))]
	{
		if SIGNALFD.with(|x| x.borrow().is_none()) {
			let mask = mask_signals();
			let flags = SfdFlags::SFD_NONBLOCK | SfdFlags::SFD_CLOEXEC;
			let fd = nix::sys::signalfd::signalfd(-1, &mask, flags).unwrap();
			SIGNALFD.with(|x| *x.borrow_mut() = Some(fd));
			register_proxy(fd);
		}
	}
	#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd",
	          target_os = "dragonfly", target_os = "bitrig", target_os = "macos", target_os = "ios"))]
	{
		if KQUEUE.with(|x| x.borrow().is_none()) {
			mask_signals();
			let add = EventFlag::EV_ADD | EventFlag::EV_ENABLE;
			let filt = FilterFlag::empty();
			let fd = nix::sys::event::kqueue().unwrap();
			nix::sys::event::kevent(
				fd,
				&vec![
					KEvent::new(SIGWINCH as usize, EventFilter::EVFILT_SIGNAL, add, filt, 0, 0),
					KEvent::new(SIGCHLD as usize, EventFilter::EVFILT_SIGNAL, add, filt, 0, 0),
				],
				&mut vec![],
				0,
			).unwrap();
			KQUEUE.with(|x| *x.borrow_mut() = Some(fd));
			register_proxy(fd);
		}
	}
	HANDLER.with(|x| *x.borrow_mut() = Some(handler));
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn get_signal_name() -> String {
	let signal_number = SIGINFO.with(|x| x.borrow().as_ref().unwrap().ssi_signo);
	let signal = Signal::from_c_int(signal_number as _).unwrap();
	format!("{:?}", signal)
}

#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd",
          target_os = "dragonfly", target_os = "bitrig", target_os = "macos", target_os = "ios"))]
pub fn get_signal_name() -> String {
	let signal_number = SIGNUM.with(|x| x.borrow().unwrap());
	let signal = Signal::from_c_int(signal_number as _).unwrap();
	format!("{:?}", signal)
}

struct SignalNotificationProxy {}

impl Notifiable for SignalNotificationProxy {
	fn notify(&self) {
		#[cfg(any(target_os = "linux", target_os = "android"))]
		{
			let mut buf = [0u8; SIGNALFD_SIGINFO_SIZE];
			let fd = SIGNALFD.with(|x| x.borrow().as_ref().unwrap().clone());
			nix::unistd::read(fd, &mut buf).unwrap();

			let siginfo = unsafe { ::std::mem::transmute::<_, nix::sys::signalfd::siginfo>(buf) };
			SIGINFO.with(|x| *x.borrow_mut() = Some(siginfo));
		}
		#[cfg(any(target_os = "freebsd", target_os = "openbsd", target_os = "netbsd",
		          target_os = "dragonfly", target_os = "bitrig", target_os = "macos", target_os = "ios"))]
		{
			let mut eventlist = vec![KEvent::new(0, EventFilter::EVFILT_SIGNAL, EventFlag::empty(), FilterFlag::empty(), 0, 0)];
			let fd = KQUEUE.with(|x| x.borrow().as_ref().unwrap().clone());
			nix::sys::event::kevent_ts(fd, &vec![], &mut eventlist, None).unwrap();

			if eventlist[0].filter() != EventFilter::EVFILT_SIGNAL {
				panic!("Unexpected non-signal event on signal kqueue");
			}
			SIGNUM.with(|x| *x.borrow_mut() = Some(eventlist[0].ident()));
		}
		let handler = HANDLER.with(|x| x.borrow().clone());
		if handler.is_some() {
			handler.unwrap().notify();
		}
	}
}
