#[cfg(windows)]
extern crate winresource;

#[cfg(windows)]
fn compile_resource() {
	winresource::WindowsResource::new()
		.set_icon("icon.ico")
		.compile()
		.expect("Could not compile windows resource!");
}

fn main() {
	#[cfg(windows)]
	{
		compile_resource();
	}
}
