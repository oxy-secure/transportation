thread_local! {
	static POLL: ::std::cell::RefCell<::mio::Poll> = ::std::cell::RefCell::new(::mio::Poll::new().unwrap());
	static EVENT_HANDLERS: ::std::cell::RefCell<::std::collections::BTreeMap<usize, ::std::rc::Rc<dyn Fn(::mio::Event) -> () + 'static>>> = ::std::default::Default::default();
	static TIME_CALLBACKS: ::std::cell::RefCell<::std::collections::BTreeMap<usize, TimeCallback>> = ::std::default::Default::default();

	// Starts at 1 because 0 is reserved for run_in_thread
	static TICKER: ::std::cell::RefCell<usize> = ::std::cell::RefCell::new(1);

	static EVENT_BUFFER: ::std::rc::Rc<::std::cell::RefCell<::mio::Events>> = ::std::rc::Rc::new(::std::cell::RefCell::new(::mio::Events::with_capacity(1024)));
	static REMOTE_RECEIVER: ::std::cell::RefCell<Option<::std::sync::mpsc::Receiver<Box<dyn Fn() -> () + Send + 'static>>>> = ::std::default::Default::default();
	static REMOTE_REGISTRATION: ::std::cell::RefCell<Option<::mio::Registration>> = ::std::default::Default::default();
	static SHOULD_CONTINUE: ::std::cell::RefCell<bool> = ::std::default::Default::default();
	static FLUSH_FLAG: ::std::cell::RefCell<bool> = ::std::cell::RefCell::new(false);
}

lazy_static! {
	static ref REMOTES: ::std::sync::Mutex<::std::collections::HashMap<::std::thread::ThreadId, Remote>> = ::std::default::Default::default();
}

struct TimeCallback {
	callback: ::std::rc::Rc<dyn Fn() -> () + 'static>,
	when:     ::std::time::Instant,
	interval: Option<::std::time::Duration>,
}

struct Remote {
	set_readiness: ::mio::SetReadiness,
	run_sender:    ::std::sync::mpsc::Sender<Box<dyn Fn() -> () + Send + 'static>>,
}

fn tick() -> usize {
	TICKER.with(|x| {
		let a: usize = *x.borrow();
		*x.borrow_mut() = a.checked_add(1).expect("Ran out of callback IDs");
		a
	})
}

/// Drop the existing Poll for this thread and replace it with a freshly generated poll. Also drops all registered event_handler callbacks,
/// set_timeout callbacks, and set_interval callbacks. This is (exclusively?) useful after a fork() when you want to separate yourself from the old
/// process without calling exec(). Should only be used from the main thread and when no other threads have been created. Should only be used when
/// inside the transportation event loop (i.e. when run() or run_worker() are on the stack).
pub fn flush() {
	let new_poll = ::mio::Poll::new().unwrap();
	POLL.with(|x| x.replace(new_poll));
	EVENT_HANDLERS.with(|x| x.borrow_mut().clear());
	TIME_CALLBACKS.with(|x| x.borrow_mut().clear());
	TICKER.with(|x| *x.borrow_mut() = 1);
	REMOTE_RECEIVER.with(|x| x.borrow_mut().take());
	REMOTE_REGISTRATION.with(|x| x.borrow_mut().take());
	init_loop();
	FLUSH_FLAG.with(|x| *x.borrow_mut() = true);
}

/// Insert a listener and return a unique usize value that can be used to register one or more [`Evented`](::mio::Evented)s with the internal poll
/// for this thread. Note: this callback will never be called unless [`borrow_poll`]`(|poll| poll.`[`register`](::mio::Poll::register)`())` is called
/// with the returned token.
pub fn insert_listener(listener: impl Fn(::mio::Event) -> () + 'static) -> usize {
	let idx = tick();
	EVENT_HANDLERS.with(|x| x.borrow_mut().insert(idx, ::std::rc::Rc::new(listener)));
	idx
}

/// Remove a previously registered listener. Returns true if a listener was removed and false otherwise. Note: The Evented item must still be removed
/// from the poll separately.
pub fn remove_listener(idx: usize) -> bool {
	EVENT_HANDLERS.with(|x| x.borrow_mut().remove(&idx).is_some())
}

/// Call callback once after timeout has passed. Returns an identifier that is unique across insert_listener, set_timeout, and set_interval that can
/// be used with the clear_timeout function.
pub fn set_timeout(callback: impl Fn() -> () + 'static, timeout: ::std::time::Duration) -> usize {
	let callback = ::std::rc::Rc::new(callback);
	let when = ::std::time::Instant::now() + timeout;
	let idx = tick();
	TIME_CALLBACKS.with(|x| {
		x.borrow_mut().insert(
			idx,
			TimeCallback {
				callback,
				when,
				interval: None,
			},
		)
	});
	idx
}

/// Call callback after interval time has passed, then again once every interval. Returns an identifier that is unique across insert_listener,
/// set_timeout, and set_interval that can be used with the clear_interval function.
pub fn set_interval(callback: impl Fn() -> () + 'static, interval: ::std::time::Duration) -> usize {
	let callback = ::std::rc::Rc::new(callback);
	let when = ::std::time::Instant::now() + interval;
	let idx = tick();
	TIME_CALLBACKS.with(|x| {
		x.borrow_mut().insert(
			idx,
			TimeCallback {
				callback,
				when,
				interval: Some(interval),
			},
		)
	});
	idx
}

/// Remove an existing timeout before it has occured. Returns true if a timeout was removed or false otherwise.
pub fn clear_timeout(idx: usize) -> bool {
	if TIME_CALLBACKS.with(|x| x.borrow().get(&idx).map(|y| y.interval.is_some()).unwrap_or(true)) {
		return false;
	}
	TIME_CALLBACKS.with(|x| x.borrow_mut().remove(&idx).is_some())
}

/// Remove an existing interval. Returns true if an interval was removed or false otherwise.
pub fn clear_interval(idx: usize) -> bool {
	if TIME_CALLBACKS.with(|x| x.borrow().get(&idx).map(|y| y.interval.is_none()).unwrap_or(true)) {
		return false;
	}
	TIME_CALLBACKS.with(|x| x.borrow_mut().remove(&idx).is_some())
}

fn get_soonest_timeout() -> (Option<usize>, Option<::std::time::Duration>) {
	let mut idx = None;
	let mut soonest_instant = None;
	TIME_CALLBACKS.with(|x| {
		for (k, v) in x.borrow_mut().iter() {
			if soonest_instant.is_none() || v.when < soonest_instant.unwrap() {
				idx = Some(*k);
				soonest_instant = Some(v.when);
			}
		}
	});
	if idx.is_none() {
		(None, None)
	} else {
		let now = ::std::time::Instant::now();
		if soonest_instant.unwrap() <= now {
			(idx, Some(::std::time::Duration::from_secs(0)))
		} else {
			(idx, Some(soonest_instant.unwrap().duration_since(now)))
		}
	}
}

fn dispatch_timeout(time_idx: usize) {
	let now = ::std::time::Instant::now();
	if TIME_CALLBACKS.with(|x| x.borrow().get(&time_idx).unwrap().when <= now) {
		if TIME_CALLBACKS.with(|x| x.borrow().get(&time_idx).unwrap().interval.is_none()) {
			let callback = TIME_CALLBACKS.with(|x| x.borrow_mut().remove(&time_idx).unwrap());
			(callback.callback)();
		} else {
			let callback = TIME_CALLBACKS.with(|x| {
				let interval = x.borrow().get(&time_idx).unwrap().interval.unwrap();
				x.borrow_mut().get_mut(&time_idx).unwrap().when = now + interval;
				x.borrow().get(&time_idx).unwrap().callback.clone()
			});
			(*callback)();
		}
	}
}

/// Run callback with a reference to the internal MIO poll for the current thread. Transportation keeps a separate poll for every thread and it is
/// appropriate to run transportation from multiple threads simultaneously.
pub fn borrow_poll<T, R>(callback: T) -> R
where
	T: Fn(&::mio::Poll) -> R,
{
	POLL.with(|x| (callback)(&*x.borrow()))
}

fn dispatch_event(event: ::mio::Event) {
	let token: usize = event.token().0;
	if token == 0 {
		while let Ok(callback) = REMOTE_RECEIVER.with(|x| x.borrow().as_ref().unwrap().try_recv()) {
			(*callback)();
		}
		return;
	}
	let callback = EVENT_HANDLERS.with(|x| x.borrow().get(&token).map(|x| x.clone()));
	if let Some(callback) = callback {
		(*callback)(event);
	}
}

fn empty() -> bool {
	TIME_CALLBACKS.with(|x| x.borrow().is_empty()) && EVENT_HANDLERS.with(|x| x.borrow().is_empty())
}

fn turn_internal(events: &mut ::mio::Events) {
	let (time_idx, timeout) = get_soonest_timeout();
	events.clear();
	POLL.with(|x| x.borrow().poll(events, timeout)).unwrap();

	if let Some(time_idx) = time_idx {
		dispatch_timeout(time_idx);
		if FLUSH_FLAG.with(|x| *x.borrow()) {
			FLUSH_FLAG.with(|x| *x.borrow_mut() = false);
			return;
		}
	}

	for event in events.iter() {
		dispatch_event(event);
		if FLUSH_FLAG.with(|x| *x.borrow()) {
			FLUSH_FLAG.with(|x| *x.borrow_mut() = false);
			return;
		}
	}
}

/// Blocks the thread by polling on the internal MIO poll, dispatching events to event callbacks and calling set_timeout/set_interval callbacks at
/// the appropriate times. Returns when there are no remaining registered time or event callbacks. Panics if called while transportation is already
/// running in this same thread.
pub fn run() {
	init_loop();
	let events_rc = EVENT_BUFFER.with(|x| x.clone());
	let events: &mut ::mio::Events = &mut *events_rc.borrow_mut();
	while SHOULD_CONTINUE.with(|x| *x.borrow()) {
		if empty() {
			return;
		}
		turn_internal(events);
	}
}

/// Exactly the same as [`run`], but does not exit when empty. This is useful for threadpool workers (see [`run_in_thread`])
pub fn run_worker() {
	init_loop();
	let events_rc = EVENT_BUFFER.with(|x| x.clone());
	let events: &mut ::mio::Events = &mut *events_rc.borrow_mut();
	while SHOULD_CONTINUE.with(|x| *x.borrow()) {
		turn_internal(events);
	}
}

fn init_loop() {
	SHOULD_CONTINUE.with(|x| *x.borrow_mut() = true);

	if REMOTE_RECEIVER.with(|x| x.borrow().is_some()) {
		return;
	}

	let (tx, rx) = ::std::sync::mpsc::channel();
	let thread_id = ::std::thread::current().id();
	let (registration, set_readiness) = ::mio::Registration::new2();
	REMOTE_RECEIVER.with(|x| *x.borrow_mut() = Some(rx));
	borrow_poll(|poll| {
		poll.register(&registration, ::mio::Token(0), ::mio::Ready::readable(), ::mio::PollOpt::edge())
			.unwrap()
	});
	REMOTE_REGISTRATION.with(|x| *x.borrow_mut() = Some(registration));
	let mut remotes_lock = REMOTES.lock().unwrap();
	remotes_lock.insert(
		thread_id,
		Remote {
			set_readiness,
			run_sender: tx,
		},
	);
}

/// Run a callback in a remote thread that is owned by transportation. Spins until the remote thread calls [`run`] or [`run_worker`]. May deadlock if
/// the remote thread doesn't enter the transportation event loop. May return `Err(())` if the remote thread has terminated.
pub fn run_in_thread(thread_id: ::std::thread::ThreadId, callback: impl Fn() -> () + Send + 'static) -> Result<(), ()> {
	let callback = Box::new(callback);
	loop {
		let remotes = REMOTES.lock().unwrap();
		let remote = match remotes.get(&thread_id) {
			Some(remote) => remote,
			None => {
				::std::thread::yield_now();
				continue;
			}
		};
		remote.run_sender.send(callback).map_err(|_| ())?;
		remote.set_readiness.set_readiness(::mio::Ready::readable()).map_err(|_| ())?;
		return Ok(());
	}
}

/// Causes the event loop to exit the next time it would iterate. The event loop may be resumed with [`run()`] or [`run_worker()`]
pub fn stop() {
	SHOULD_CONTINUE.with(|x| *x.borrow_mut() = false);
}
