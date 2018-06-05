use mio::{Event, Events, Poll};
use notify::Notifiable;
use std::{
	cell::RefCell, collections::HashMap, rc::Rc, time::{Duration, Instant},
};

thread_local! {
	pub static POLL: Poll = Poll::new().unwrap();
	static LISTENERS: RefCell<HashMap<usize, Rc<Notifiable>>> = RefCell::new(HashMap::new());
	pub static EVENT: RefCell<Option<Event>> = RefCell::new(None);
	static TIMECALLBACKS: RefCell<Vec<(Instant, Rc<Notifiable>)>> = RefCell::new(Vec::new());
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

pub fn borrow_poll<T>(callback: T)
where
	T: FnOnce(&Poll),
{
	POLL.with(|x| callback(&*x));
}

pub fn remove_listener(key: usize) {
	LISTENERS.with(|x| x.borrow_mut().remove(&key));
}

pub fn insert_listener(listener: Rc<Notifiable>) -> usize {
	let key = LISTENERS.with(|x| find_key(x.borrow_mut().keys().cloned().collect()));
	LISTENERS.with(|x| x.borrow_mut().insert(key, listener));
	key
}

fn handle_event(event: Event) {
	EVENT.with(|x| *x.borrow_mut() = Some(event));
	let key: usize = event.token().0;
	let handler = LISTENERS.with(|x| x.borrow_mut().get(&key).unwrap().clone());
	handler.notify();
}

pub fn get_event() -> Event {
	EVENT.with(|x| x.borrow_mut().unwrap())
}

fn empty() -> bool {
	LISTENERS.with(|x| x.borrow_mut().keys().len()) == 0 && TIMECALLBACKS.with(|x| x.borrow().len()) == 0
}

pub fn run() -> ! {
	trace!("Transportation main-loop started");
	let mut events = Events::with_capacity(1024);
	loop {
		if empty() {
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
		POLL.with(|x| x.poll(&mut events, duration).unwrap());
		if duration.is_some() && now.elapsed() >= duration.unwrap() {
			TIMECALLBACKS.with(|x| x.borrow_mut().remove(remove_idx));
			timecallback.unwrap().notify();
		}
		for event in events.iter() {
			handle_event(event);
		}
	}
}
