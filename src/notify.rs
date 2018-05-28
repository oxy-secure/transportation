use std::rc::Rc;

pub trait Notifiable {
	fn notify(&self);
}

pub trait Notifies {
	fn set_notify(&self, callback: Rc<Notifiable>);
}
