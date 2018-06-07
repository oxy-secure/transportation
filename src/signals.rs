use mio::{unix::EventedFd, PollOpt, Ready, Token};
use nix::{
	self, sys::{
		signal::Signal::{self, *}, signalfd::{siginfo, SfdFlags, SIGNALFD_SIGINFO_SIZE},
	},
};
use std::{cell::RefCell, os::unix::io::RawFd, rc::Rc};
use Notifiable;

thread_local! {
	static SIGNALFD: RefCell<Option<RawFd>> = RefCell::new(None);
	static HANDLER: RefCell<Option<Rc<Notifiable>>> = RefCell::new(None);
	static SIGINFO: RefCell<Option<siginfo>> = RefCell::new(None);
}

pub fn set_signal_handler(handler: Rc<Notifiable>) {
	if SIGNALFD.with(|x| x.borrow().is_none()) {
		let mut mask = nix::sys::signal::SigSet::empty();
		mask.add(SIGWINCH);
		mask.add(SIGCHLD);
		mask.thread_block().unwrap();
		let flags = SfdFlags::SFD_NONBLOCK | SfdFlags::SFD_CLOEXEC;
		let fd = nix::sys::signalfd::signalfd(-1, &mask, flags).unwrap();
		SIGNALFD.with(|x| *x.borrow_mut() = Some(fd));
		let proxy = SignalNotificationProxy {};
		let token = ::insert_listener(Rc::new(proxy));
		::borrow_poll(|x| {
			x.register(&EventedFd(&fd), Token(token), Ready::readable(), PollOpt::level()).unwrap();
		});
	}
	HANDLER.with(|x| *x.borrow_mut() = Some(handler));
}

pub fn get_signal_name() -> String {
	let signal_number = SIGINFO.with(|x| x.borrow().as_ref().unwrap().ssi_signo);
	let signal = Signal::from_c_int(signal_number as _).unwrap();
	format!("{:?}", signal)
}

struct SignalNotificationProxy {}

impl Notifiable for SignalNotificationProxy {
	fn notify(&self) {
		let mut buf = [0u8; SIGNALFD_SIGINFO_SIZE];
		let fd = SIGNALFD.with(|x| x.borrow().as_ref().unwrap().clone());
		nix::unistd::read(fd, &mut buf).unwrap();

		let siginfo = unsafe { ::std::mem::transmute::<_, nix::sys::signalfd::siginfo>(buf) };
		SIGINFO.with(|x| *x.borrow_mut() = Some(siginfo));
		let handler = HANDLER.with(|x| x.borrow().clone());
		if handler.is_some() {
			handler.unwrap().notify();
		}
	}
}
