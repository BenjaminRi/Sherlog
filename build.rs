#[cfg(windows)]
extern crate windres;

#[cfg(windows)]
fn compile_resource() {
	//let version = env!("CARGO_PKG_VERSION");
	// TODO:
	//- Rename version.rc to version.rc.in
	//- On build, copy version.rc.in to version.rc
	//- Fill version.rc with version data from Cargo.toml env variables
	//- Handle #ifdef _DEBUG in version.rc too!
	/*let profile = std::env::var("PROFILE").unwrap();
    match profile.as_str() {
        "debug" => (),
        "release" => (),
        _ => (),
    }*/
	windres::Build::new().compile("version.rc").unwrap();
}

fn main() {
	#[cfg(windows)] {
		compile_resource();
	}
}
