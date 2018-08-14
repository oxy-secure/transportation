extern crate transportation;

fn main() {
	transportation::set_timeout(|| eprintln!("Hello"), ::std::time::Duration::from_secs(5));
	transportation::run();
	eprintln!("Done");
}
