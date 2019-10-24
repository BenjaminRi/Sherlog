extern crate zip;

use super::super::model;
use super::glog;

pub fn from_file(path : &std::path::PathBuf) -> Result<model::LogSource, std::io::Error> {
	let file = std::fs::File::open(&path).unwrap();
	let mut archive = zip::ZipArchive::new(file).unwrap();
	
	let mut child_sources = Vec::new();
	child_sources.reserve(archive.len());
	for i in 0..archive.len() {
		let file = archive.by_index_decrypt(i, "test".as_bytes()).unwrap();
		let outpath = file.sanitized_name();
		let stem = outpath.file_stem().unwrap();
		let stem = stem.to_string_lossy();
		
		println!("File contained: {}", &stem);
		if let Some(extension) = outpath.extension() {
			match extension.to_string_lossy().as_ref() {
				// ../logfiles/example.glog
				"glog" => {
					//let glog_entries = Vec::new();
					let glog_entries = glog::to_log_entries(file);
					let child_source = model::LogSource {name: stem.to_string(), children: {model::LogSourceContents::Entries(glog_entries) } };
					child_sources.push(child_source);
				},
				_ => (),
			}
		}
	}
	
	Ok(model::LogSource {name: "example2_1".to_string(), children: {model::LogSourceContents::Sources(child_sources) } })
}