use super::super::model;
use super::glog;
use super::sfile;

use std::fs::File;

#[derive(Debug)]
pub enum LogParseError {
	IoError(std::io::Error),
	UnrecognizedFileExtension(std::ffi::OsString),
	NoFileExtension,
}

impl std::error::Error for LogParseError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			LogParseError::IoError(err) => Some(err),
			_ => None,
		}
	}
}

impl std::fmt::Display for LogParseError {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			LogParseError::IoError(err) => write!(f, "{}", err),
			LogParseError::UnrecognizedFileExtension(ext) => {
				write!(f, "Unrecognized file extension: {}", ext.to_string_lossy())
			}
			LogParseError::NoFileExtension => write!(f, "No file extension"),
		}
	}
}

impl From<std::io::Error> for LogParseError {
	fn from(error: std::io::Error) -> Self {
		LogParseError::IoError(error)
	}
}

pub fn from_file(path: &std::path::PathBuf) -> Result<model::LogSource, LogParseError> {
	let extension = path.extension();
	if let Some(extension) = extension {
		match extension.to_string_lossy().to_lowercase().as_ref() {
			// ../logfiles/example.glog
			"glog" => {
				let file = File::open(&path)?;
				let root = model::LogSource {
					name: path.file_name().unwrap().to_string_lossy().to_string(),
					children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
				};
				Ok(glog::to_log_entries(file, root))
			}
			// ../logfiles/logfile1.sfile
			"sfile" | "lfile" => sfile::from_file(&path).map_err(LogParseError::IoError),
			//TODO: Implement heuristic, more file types
			_ => Err(LogParseError::UnrecognizedFileExtension(
				extension.to_os_string(),
			)),
		}
	} else {
		//TODO: Implement heuristic
		Err(LogParseError::NoFileExtension)
	}
}
