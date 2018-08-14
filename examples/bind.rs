extern crate transportation;

fn main() {
	let interval_token = ::transportation::set_interval(|| eprintln!("Interval"), ::std::time::Duration::from_secs(5));
	::transportation::set_timeout(
		move || {
			eprintln!("Timeout 1");
			let token = ::transportation::set_timeout(|| eprintln!("Timeout 2"), ::std::time::Duration::from_secs(5));
			::transportation::set_timeout(
				move || {
					::transportation::clear_interval(interval_token);
				},
				::std::time::Duration::from_secs(20),
			);
			::transportation::clear_timeout(token);
		},
		::std::time::Duration::from_secs(7),
	);
	let listener = ::std::rc::Rc::new(::transportation::mio::net::TcpListener::bind(&"0.0.0.0:9912".parse().unwrap()).unwrap());
	let listener2 = listener.clone();
	let token = ::transportation::insert_listener(move |event| {
		let listener = listener2.clone();
		let (_socket, address) = listener.accept().unwrap();
		eprintln!("Accepted connection from {:?}, {:?}", address, event);
	});
	::transportation::borrow_poll(|x| {
		x.register(
			&*listener,
			::transportation::mio::Token(token),
			::transportation::mio::Ready::readable(),
			::transportation::mio::PollOpt::level(),
		).unwrap()
	});
	::transportation::run();
}
