extern crate transportation;

use std::rc::Rc;

fn main() {
	transportation::set_timeout(
		Rc::new(|| {
			println!("Hello, world.");
		}),
		std::time::Duration::from_secs(5),
	);
	transportation::run();
}
