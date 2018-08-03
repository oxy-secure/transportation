fn main() {
	let token = ::transportation::insert_callback(|evt| eprintln!("x: {:?}, {:?}", evt, ::std::thread::current().id()));
	let poll = ::transportation::get_poll();
	poll.register(
		&::mio::unix::EventedFd(&0),
		::mio::Token(token),
		::mio::Ready::readable(),
		::mio::PollOpt::edge(),
	).unwrap();
	::transportation::set_interval(
		|| eprintln!("Banana! {:?}", ::std::thread::current().id()),
		::std::time::Duration::from_secs(1),
	);
	::transportation::set_timeout(
		|| eprintln!("One-off {:?}", ::std::thread::current().id()),
		::std::time::Duration::from_secs(5),
	);
	::transportation::run(3);
}
