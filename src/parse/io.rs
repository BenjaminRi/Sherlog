use super::super::model;
use super::glog;
use super::sfile;

use std::fs::File;

pub fn from_file(path : &std::path::PathBuf) -> Result<model::LogSource, std::io::Error> {
	let extension = path.extension();
	if let Some(extension) = extension {
		match extension.to_string_lossy().as_ref() {
			// ../logfiles/example.glog
			"glog" => {
				let file = File::open(&path)?;
				let log_entries = glog::to_log_entries(file);
				Ok(model::LogSource {name: "example2_1".to_string(), children: {model::LogSourceContents::Entries(log_entries) } })
			},
			// ../logfiles/logfile1.sfile
			"sfile" => {
				sfile::from_file(&path)
			}
			_ => panic!("Unknown file extension: {}!", extension.to_string_lossy()), //TODO: Implement heuristic, more file types
		}
	} else {
		//TODO: Implement heuristic
		panic!("No file extension!");
	}
}