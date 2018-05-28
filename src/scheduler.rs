use mio::{Event, Events, Poll};
use notify::Notifiable;
use std::{cell::RefCell, collections::HashMap, rc::Rc};

thread_local! {
	pub static POLL: Poll = Poll::new().unwrap();
	static LISTENERS: RefCell<HashMap<usize, Rc<Notifiable>>> = RefCell::new(HashMap::new());
	pub static EVENT: RefCell<Option<Event>> = RefCell::new(None);
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
	LISTENERS.with(|x| x.borrow_mut().keys().len()) == 0
}

pub fn run() -> ! {
	trace!("Transportation main-loop started");
	let mut events = Events::with_capacity(1024);
	loop {
		if empty() {
			::std::process::exit(0);
		}
		POLL.with(|x| x.poll(&mut events, None).unwrap());
		for event in events.iter() {
			handle_event(event);
		}
	}
}
