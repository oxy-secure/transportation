extern crate transportation;

fn main() {
	let child = ::std::thread::spawn(|| ::transportation::run_worker()).thread().id();
	::transportation::run_in_thread(child, || {
		::transportation::set_timeout(|| eprintln!("Child!"), ::std::time::Duration::from_secs(1));
	}).unwrap();
	::std::thread::sleep(::std::time::Duration::from_secs(2));
	::transportation::run_in_thread(child, || ::transportation::stop()).unwrap();
	::std::thread::sleep(::std::time::Duration::from_secs(2));
	eprintln!("{:?}", ::transportation::run_in_thread(child, || eprintln!("Alive")));
}
