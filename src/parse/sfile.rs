extern crate zip;

use super::super::model;
use super::glog;
use super::xlog;

use std::mem;
use std::collections::HashMap;

static SFILE_PASSWORD: Option<&'static str> = option_env!("SFILE_PASSWORD");

pub fn from_file(path: &std::path::PathBuf) -> Result<model::LogSource, std::io::Error> {
	let file = std::fs::File::open(&path)?;
	let mut archive = zip::ZipArchive::new(file).unwrap(); //TODO: 21.06.2020: Handle ZipError!;

	let mut glog_files = Vec::new();

	let mut client_child_sources = Vec::new();
	let mut child_sources = Vec::new();
	child_sources.reserve(archive.len());
	for i in 0..archive.len() {
		let file = if let Some(password) = SFILE_PASSWORD {
			archive.by_index_decrypt(i, password.as_bytes()).unwrap() //TODO: 21.06.2020: Handle ZipError!
		} else {
			archive.by_index(i).unwrap() //TODO: 21.06.2020: Handle ZipError!
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
				"xlog" => {
					//println!("XLOG: {}", &stem);
					let root = model::LogSource {
						name: stem.to_string(),
						children: { model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) },
					};
					client_child_sources.push(xlog::to_log_entries(file, root));
				}
				_ => (),
			}
		}
	}
	
	//Arrange Client logs into their respective channels
	let mut client_log_sources = HashMap::<String, model::LogSource>::new();
	for file_source in client_child_sources {
		let split_name = file_source.name.split("_");
		let channel_name = if let Some(channel) = split_name.skip(2).take(1).next() {
			channel
		} else {
			"Unknown"
		};
		
		let source_option = client_log_sources.get_mut(channel_name);
		if let Some(source) = source_option {
			//Log sub-source exists, push contents
			let children = &mut source.children;
			match children {
				model::LogSourceContents::Entries(v) => {
					if let model::LogSourceContents::Entries(mut entries) = file_source.children {
						v.append(&mut entries);
					} else {
						unreachable!(); //If this panics, there is a bug in the XLOG parser
					}
				}
				_ => unreachable!(), //We only insert LogSourceContents::Entries
			}
		} else {
			//Log sub-source does not yet exist
			client_log_sources.insert(
				channel_name.to_string(),
				model::LogSource {
					name: channel_name.to_string(),
					children: {
						if let model::LogSourceContents::Entries(entries) = file_source.children {
							model::LogSourceContents::Entries(entries)
						} else {
							unreachable!(); //If this panics, there is a bug in the XLOG parser
						}
					},
				},
			);
		}
	}
	let mut client_child_sources = Vec::new();
	for (_, sub_source) in client_log_sources {
		client_child_sources.push(sub_source);
	}
	
	//client_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	

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
	let mut sensor_child_sources: std::vec::Vec<model::LogSource> = Vec::new();
	let mut unknown_child_sources = Vec::new();

	for mut source in child_sources {
		//Controller logs
		if source.name.starts_with("contr_") {
			//Remove "contr_" to make it look nicer
			source.name = source.name.split_off(6);
			contr_child_sources.push(source);
			continue;
		}

		//Sensor logs
		let mut iter = source.name.splitn(2, '_');
		let board_name = iter.next().unwrap(); //First element always exists
		if let Some(log_name) = iter.next() {
			if board_name == "adm" || //G
				board_name == "axis" ||
				board_name == "laseroven" || //G
				board_name == "sensorbase" ||
				board_name == "telescope" ||
				board_name == "trigger" ||
				board_name == "wfd"
			//W
			{
				if sensor_child_sources.is_empty()
					|| !(sensor_child_sources.last().unwrap().name == board_name)
				{
					let board_name_string = board_name.to_string();
					source.name = log_name.to_string();
					sensor_child_sources.push(model::LogSource {
						name: board_name_string,
						children: { model::LogSourceContents::Sources(vec![source]) },
					});
				} else {
					source.name = log_name.to_string();
					if let model::LogSourceContents::Sources(sources) =
						&mut sensor_child_sources.last_mut().unwrap().children
					{
						sources.push(source)
					} else {
						//We only pushed model::LogSourceContents::Sources
						unreachable!();
					}
				}
				continue;
			}
		}

		//Unknown logs
		unknown_child_sources.push(source);
	}
	
	//Case insensitive sort by log source name
	contr_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	sensor_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	unknown_child_sources.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

	let contr_logs = model::LogSource {
		name: "Controller".to_string(),
		children: { model::LogSourceContents::Sources(contr_child_sources) },
	};
	let sensor_logs = model::LogSource {
		name: "Sensor".to_string(),
		children: { model::LogSourceContents::Sources(sensor_child_sources) },
	};
	let client_logs = model::LogSource {
		name: "Client".to_string(),
		children: { model::LogSourceContents::Sources(client_child_sources) },
	};

	let mut sources_vec = vec![client_logs, contr_logs, sensor_logs];
	if !unknown_child_sources.is_empty() {
		let unknown_logs = model::LogSource {
			name: "Unknown".to_string(),
			children: { model::LogSourceContents::Sources(unknown_child_sources) },
		};
		sources_vec.push(unknown_logs);
	}

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
							self.archive.by_index_decrypt(idx, password.as_bytes()).unwrap() //TODO: 21.06.2020: Handle ZipError!
						} else {
							self.archive.by_index(idx).unwrap() //TODO: 21.06.2020: Handle ZipError!
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
