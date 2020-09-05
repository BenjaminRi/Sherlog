#[cfg(windows)]
extern crate winres;

#[cfg(windows)]
fn compile_resource() {
	winres::WindowsResource::new()
		.set_icon("icon.ico")
		.compile()
		.expect("Could not compile windows resource!");
}

fn main() {
	#[cfg(windows)] {
		compile_resource();
	}
}
