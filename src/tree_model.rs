extern crate gtk_sys;
extern crate gtk;

//use glib::translate::*;
use gtk::subclass::prelude::*;

pub trait SeekableImpl: ObjectImpl + 'static {
}