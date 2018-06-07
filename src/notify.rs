use std::rc::Rc;

pub trait Notifiable {
	fn notify(&self);
}

pub trait Notifies {
	fn set_notify(&self, callback: Rc<Notifiable>);
}

impl<T> Notifiable for T
where
	T: Fn() -> (),
{
	fn notify(&self) {
		self();
	}
}
