use mio::{Event, Events, Poll};
use notify::Notifiable;
use std::{
	cell::RefCell,
	collections::HashMap,
	rc::Rc,
	time::{Duration, Instant},
};

thread_local! {
	static POLL: RefCell<Option<Poll>> = ::std::default::Default::default();
	static LISTENERS: RefCell<HashMap<usize, Rc<Notifiable>>> = ::std::default::Default::default();
	pub static EVENT: RefCell<Option<Event>> = ::std::default::Default::default();
	static TIMECALLBACKS: RefCell<Vec<(Instant, Rc<Notifiable>)>> = ::std::default::Default::default();
}

pub fn set_timeout(callback: Rc<Notifiable>, duration: Duration) {
	let when = Instant::now() + duration;
	TIMECALLBACKS.with(|x| x.borrow_mut().push((when, callback)));
}

fn find_key(mut existing_keys: Vec<usize>) -> usize {
	existing_keys.sort();
	if let Some((k, _)) = existing_keys.iter().enumerate().filter(|(a, b)| a != *b).next() {
		return k;
	}
	existing_keys.len()
}

pub fn borrow_poll<T, R>(callback: T) -> R
where
	T: FnOnce(&Poll) -> R,
{
	POLL.with(|x| {
		if x.borrow().is_none() {
			*x.borrow_mut() = Some(Poll::new().unwrap());
		}
	});
	POLL.with(|x| callback(&x.borrow().as_ref().unwrap()))
}

pub fn remove_listener(key: usize) {
	let result = LISTENERS.with(|x| x.borrow_mut().remove(&key));
	if result.is_none() {
		debug!("Attempted to remove a non-existent listener");
	}
}

pub fn insert_listener(listener: Rc<Notifiable>) -> usize {
	let key = LISTENERS.with(|x| find_key(x.borrow_mut().keys().cloned().collect()));
	LISTENERS.with(|x| x.borrow_mut().insert(key, listener));
	key
}

fn handle_event(event: Event) {
	EVENT.with(|x| *x.borrow_mut() = Some(event));
	let key: usize = event.token().0;
	let handler = LISTENERS.with(|x| x.borrow_mut().get(&key).map(|x| x.clone()));
	if handler.is_none() {
		warn!("Event with no handler: {:?}", event);
		return;
	}
	handler.unwrap().notify();
}

pub fn get_event() -> Event {
	EVENT.with(|x| x.borrow_mut().unwrap())
}

pub fn flush() {
	POLL.with(|x| *x.borrow_mut() = None);
	LISTENERS.with(|x| x.borrow_mut().clear());
	TIMECALLBACKS.with(|x| x.borrow_mut().clear());
	EVENT.with(|x| x.borrow_mut().take());
	::signals::flush();
}

fn empty() -> bool {
	LISTENERS.with(|x| x.borrow_mut().keys().len()) == 0 && TIMECALLBACKS.with(|x| x.borrow().len()) == 0
}

pub fn run() -> ! {
	trace!("Transportation main-loop started");
	let mut events = Events::with_capacity(1024);
	loop {
		if empty() {
			trace!("Exiting due to empty condition.");
			::std::process::exit(0);
		}
		let now = Instant::now();
		let mut timecallback = None;
		let mut duration = None;
		let mut remove_idx = 0;
		TIMECALLBACKS.with(|x| {
			for (k, v) in x.borrow().iter().enumerate() {
				if v.0 < now {
					timecallback = Some(v.1.clone());
					duration = Some(Duration::from_secs(0));
					remove_idx = k;
					return;
				}
				if duration.is_none() || v.0.duration_since(now) < duration.unwrap() {
					timecallback = Some(v.1.clone());
					duration = Some(v.0.duration_since(now));
					remove_idx = k;
				}
			}
		});
		borrow_poll(|x| x.poll(&mut events, duration).unwrap());
		if duration.is_some() && now.elapsed() >= duration.unwrap() {
			TIMECALLBACKS.with(|x| x.borrow_mut().remove(remove_idx));
			timecallback.unwrap().notify();
		}
		for event in events.iter() {
			handle_event(event);
		}
	}
}
