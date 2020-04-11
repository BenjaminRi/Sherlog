extern crate zip;

use super::super::model;
use super::glog;
use std::mem;

static SFILE_PASSWORD: Option<&'static str> = option_env!("SFILE_PASSWORD");

pub fn from_file(path: &std::path::PathBuf) -> Result<model::LogSource, std::io::Error> {
	let file = std::fs::File::open(&path).unwrap();
	let mut archive = zip::ZipArchive::new(file).unwrap();

	let mut glog_files = Vec::new();

	let mut child_sources = Vec::new();
	child_sources.reserve(archive.len());
	for i in 0..archive.len() {
		let file = if let Some(password) = SFILE_PASSWORD {
			archive.by_index_decrypt(i, password.as_bytes()).unwrap()
		} else {
			archive.by_index(i).unwrap()
		};
		let outpath = file.sanitized_name();
		let stem = outpath.file_stem().unwrap();
		let stem = stem.to_string_lossy();

		//println!("File contained: {}", &stem);
		if let Some(extension) = outpath.extension() {
			match extension.to_string_lossy().as_ref() {
				"glog" => {
					let mut s = stem.to_string();
					strip_suffix(&mut s);
					glog_files.push(ZipEntry { name: s, index: i });
				}
				_ => (),
			}
		}
	}

	//Merge numbered files together (e.g. contr_ProcessManager and contr_ProcessManager_1)
	//Note: The number was already stripped with strip_suffix.
	glog_files.sort_unstable();
	let mut deque = std::collections::VecDeque::new();
	let mut last_name = "".to_string();
	for file in glog_files {
		if last_name != file.name {
			if !deque.is_empty() {
				let deque = mem::replace(&mut deque, std::collections::VecDeque::new());
				let reader = ConcatZipReader::new(&mut archive, deque);
				let root = model::LogSource {
					name: last_name,
					children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
				};
				child_sources.push(glog::to_log_entries(reader, root));
			}
			println!("--------------------");
			println!("Glog file: {:?}", file);
			last_name = file.name;
		} else {
			println!("Glog file: {:?}", file);
		}
		deque.push_back(file.index);
	}
	if !deque.is_empty() {
		let deque = mem::replace(&mut deque, std::collections::VecDeque::new());
		let reader = ConcatZipReader::new(&mut archive, deque);
		let root = model::LogSource {
			name: last_name,
			children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
		};
		child_sources.push(glog::to_log_entries(reader, root));
	}
	
	let mut contr_child_sources = Vec::new();
	let mut sensor_child_sources = Vec::new();
	let mut unknown_child_sources = Vec::new();
	
	for source in child_sources {
		if source.name.starts_with("contr_") {
			contr_child_sources.push(source);
		} else if 
			source.name.starts_with("adm_") || //G
			source.name.starts_with("axis_") ||
			source.name.starts_with("laseroven_") || //G
			source.name.starts_with("sensorbase_") ||
			source.name.starts_with("telescope_") ||
			source.name.starts_with("trigger_") ||
			source.name.starts_with("wfd_") //W
		{
			sensor_child_sources.push(source);
		} else {
			unknown_child_sources.push(source);
		}
	}
	
	let contr_logs = model::LogSource {
		name: "Controller".to_string(),
		children: { model::LogSourceContents::Sources(contr_child_sources) },
	};
	let sensor_logs = model::LogSource {
		name: "Sensor".to_string(),
		children: { model::LogSourceContents::Sources(sensor_child_sources) },
	};
	
	let sources_vec = {
		if unknown_child_sources.is_empty() {
			vec![contr_logs, sensor_logs]
		}
		else
		{
			let unknown_logs = model::LogSource {
				name: "Unknown".to_string(),
				children: { model::LogSourceContents::Sources(unknown_child_sources) },
			};
			vec![contr_logs, sensor_logs, unknown_logs]
		}
	};

	Ok(model::LogSource {
		name: path.file_name().unwrap().to_string_lossy().to_string(),
		children: { model::LogSourceContents::Sources(sources_vec) },
	})
}

struct ConcatZipReader<'a, R: std::io::Read + std::io::Seek> {
	archive: &'a mut zip::ZipArchive<R>,
	file: Option<zip::read::ZipFile<'a>>,
	indices: std::collections::VecDeque<usize>,
}

impl<'a, R: std::io::Read + std::io::Seek> ConcatZipReader<'a, R> {
	fn new(
		archive: &'a mut zip::ZipArchive<R>,
		indices: std::collections::VecDeque<usize>,
	) -> ConcatZipReader<'a, R> {
		ConcatZipReader {
			archive: archive,
			file: None,
			indices: indices,
		}
	}
}

impl<'a, R: std::io::Read + std::io::Seek> std::io::Read for ConcatZipReader<'a, R> {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		loop {
			if let Some(f) = &mut self.file {
				let result = f.read(buf);
				if let Ok(bytes) = result {
					if bytes == 0 {
						//file exhausted, retry with next file
						self.file = None;
						continue;
					}
				}
				//ordinary read from the file
				return result;
			} else {
				match self.indices.pop_front() {
					Some(idx) => {
						//Need to open new file
						let f = if let Some(password) = SFILE_PASSWORD {
							self.archive.by_index_decrypt(idx, password.as_bytes())?
						} else {
							self.archive.by_index(idx)?
						};
						unsafe {
							//Due to the fact that file references archive and both are in the same struct,
							//this cannot be done in safe Rust
							self.file = Some(std::mem::transmute::<
								zip::read::ZipFile<'_>,
								zip::read::ZipFile<'a>,
							>(f));
						}
						//retry with newly opened file
						continue;
					}
					None => {
						//file list exhausted, end of reader
						return Ok(0);
					}
				}
			}
		}
	}
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ZipEntry {
	name: String,
	index: usize,
}

fn strip_suffix(s: &mut String) {
	let storage_type = if s.ends_with("_v") {
		//Virtual (in RAM)
		"_v"
	} else if s.ends_with("_p") {
		//Persistent (on Flash storage)
		"_p"
	} else {
		//Unknown storage type
		""
	};

	s.truncate(s.len() - storage_type.len());

	if let Some(offset) = s.rfind('_') {
		if let Ok(_) = &s[offset + 1..s.len()].parse::<u32>() {
			//Numbered logfile. Discard ring buffer index.
			s.truncate(offset);
		}
	}

	if s.ends_with("_ov") {
		//Overview logs are merged with normal logs.
		s.truncate(s.len() - "_ov".len());
	}

	//Restore storage type suffix
	s.push_str(storage_type);
}
