type Callback = dyn Fn(::mio::Event) -> () + Send + Sync + 'static;
type TimeCallback = dyn Fn() -> () + Send + Sync + 'static;

struct TimeCallbackDescriptor {
	callback:  ::std::sync::Arc<TimeCallback>,
	next_call: ::std::time::Instant,
	interval:  Option<::std::time::Duration>,
}

::lazy_static::lazy_static! {
	static ref POLL: ::std::sync::Mutex<Option<::std::sync::Arc<::mio::Poll>>> = ::std::default::Default::default();
	static ref CALLBACKS: ::std::sync::Mutex<::std::collections::HashMap<usize, ::std::sync::Arc<Callback>>> = ::std::default::Default::default();
	static ref TIMECALLBACKS: ::std::sync::Mutex<::std::collections::HashMap<usize, TimeCallbackDescriptor>> = ::std::default::Default::default();
	static ref TICKER: ::std::sync::atomic::AtomicUsize = ::std::sync::atomic::AtomicUsize::new(1);
	static ref STARTED: ::std::sync::atomic::AtomicBool = ::std::default::Default::default();
	static ref WAKEHANDLE: (::mio::Registration, ::mio::SetReadiness) = ::mio::Registration::new2();
}

pub fn insert_callback(callback: impl Fn(::mio::Event) -> () + Send + Sync + 'static) -> usize {
	insert_wrapped_callback(::std::sync::Arc::new(callback))
}

fn tick() -> usize {
	let idx = TICKER.fetch_add(1, ::std::sync::atomic::Ordering::Relaxed);
	if idx > ::std::usize::MAX - 1000 {
		panic!("Out of indexes");
	}
	idx
}

fn insert_wrapped_callback(callback: ::std::sync::Arc<Callback>) -> usize {
	let mut callbacks_lock = CALLBACKS.lock().unwrap();
	let idx = tick();
	let prev = callbacks_lock.insert(idx, callback);
	assert!(prev.is_none());
	idx
}

pub fn get_poll() -> ::std::sync::Arc<::mio::Poll> {
	let mut poll_lock = POLL.lock().unwrap();
	if poll_lock.is_none() {
		*poll_lock = Some(::std::sync::Arc::new(::mio::Poll::new().unwrap()));
	}
	poll_lock.as_ref().unwrap().clone()
}

pub fn set_timeout(callback: impl Fn() -> () + Send + Sync + 'static, timeout: ::std::time::Duration) -> usize {
	set_timeout_wrapped(::std::sync::Arc::new(callback), timeout, None)
}

pub fn set_interval(callback: impl Fn() -> () + Send + Sync + 'static, timeout: ::std::time::Duration) -> usize {
	set_timeout_wrapped(::std::sync::Arc::new(callback), timeout, Some(timeout))
}

fn set_timeout_wrapped(callback: ::std::sync::Arc<TimeCallback>, timeout: ::std::time::Duration, interval: Option<::std::time::Duration>) -> usize {
	let ticket = tick();
	let now = ::std::time::Instant::now();
	let next_call = now + timeout;
	let descriptor = TimeCallbackDescriptor {
		callback,
		next_call,
		interval,
	};
	{
		let mut timecallbacks_lock = TIMECALLBACKS.lock().unwrap();
		timecallbacks_lock.insert(ticket, descriptor);
	}
	wake();
	ticket
}

pub fn run_auto() {
	run(::num_cpus::get());
}

fn wake() {
	WAKEHANDLE.1.set_readiness(::mio::Ready::readable()).unwrap();
}

pub fn run(thread_count: usize) -> ! {
	assert!(thread_count >= 1);
	if STARTED.swap(true, ::std::sync::atomic::Ordering::Relaxed) {
		panic!("Attempted to launch transportation twice.");
	}
	get_poll()
		.register(&WAKEHANDLE.0, ::mio::Token(0), ::mio::Ready::readable(), ::mio::PollOpt::edge())
		.unwrap();
	for _ in 0..(thread_count - 1) {
		::std::thread::spawn(|| run_thread());
	}
	run_thread();
}

fn do_pending_timecallback() {
	let now = ::std::time::Instant::now();
	let mut earliest = now;
	let mut idx = None;
	let mut callback = None;
	{
		let mut timecallbacks_lock = TIMECALLBACKS.lock().unwrap();
		for (k, v) in &*timecallbacks_lock {
			if v.next_call <= earliest {
				earliest = v.next_call;
				idx = Some(*k);
			}
		}
		if idx.is_some() {
			callback = timecallbacks_lock.remove(&idx.unwrap());
		}
	}
	if let Some(mut callback) = callback {
		(callback.callback)();
		if callback.interval.is_some() {
			let next_call = ::std::time::Instant::now() + callback.interval.unwrap();
			callback.next_call = next_call;
			TIMECALLBACKS.lock().unwrap().insert(idx.unwrap(), callback);
		}
	}
}

fn get_sleep_duration() -> Option<::std::time::Duration> {
	let mut result = None;
	let now = ::std::time::Instant::now();
	for (_k, v) in &*TIMECALLBACKS.lock().unwrap() {
		if v.next_call <= now {
			return Some(::std::time::Duration::from_secs(0));
		}
		let delay = v.next_call.duration_since(now);
		if result.is_none() || delay < result.unwrap() {
			result = Some(delay)
		}
	}
	result
}

fn run_thread() -> ! {
	let mut events = ::mio::Events::with_capacity(1);
	let poll = get_poll();
	loop {
		do_pending_timecallback();
		events.clear();
		// This doesn't actually afford any parallelism, because even though you can epoll_wait() on the same epoll from multiple threads, mio has
		// internal locks. So, uh... TODO.
		poll.poll(&mut events, get_sleep_duration()).unwrap();
		for event in events.iter() {
			dispatch_event(event);
		}
	}
}

fn dispatch_event(event: ::mio::Event) {
	let token = event.token().0;
	if token == 0 {
		return;
	}
	let callback = {
		let callbacks_lock = CALLBACKS.lock().unwrap();
		callbacks_lock.get(&token).unwrap().clone()
	};
	(*callback)(event);
}
