extern crate gio;
extern crate gtk;
extern crate log;
extern crate chrono;

use gio::prelude::*;
use gtk::prelude::*;

#[allow(unused_imports)]
use gtk::{
    ApplicationWindow, ButtonsType, CellRendererPixbuf, CellRendererText, DialogFlags,
    MessageDialog, MessageType, Orientation, TreeStore, ListStore, TreeView, TreeViewColumn, WindowPosition,
};

use std::env::args;

use std::fs::File;
use std::io::{BufRead, BufReader};
use chrono::prelude::*;

#[allow(dead_code)]
enum LogLevel {
	Critical,
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
	fn to_string(&self) -> String {
		match self {
			LogLevel::Critical => "Critical".to_string(),
			LogLevel::Error => "Error".to_string(),
			LogLevel::Warning => "Warning".to_string(),
			LogLevel::Info => "Info".to_string(),
			LogLevel::Debug => "Debug".to_string(),
			LogLevel::Trace => "Trace".to_string(),
		}
	}
}

struct LogEntry {
	timestamp : chrono::DateTime<Utc>,
	severity : LogLevel,
	message : String,
}

enum LogSourceContents {
	Sources(Vec::<LogSource>),
	Entries(Vec::<LogEntry>),
}

struct LogSource {
	name : String,
	children : LogSourceContents,
}

impl Default for LogEntry {
    fn default() -> LogEntry {
        LogEntry {
            timestamp: DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
            severity : LogLevel::Error,
            message: "".to_string(),
        }
    }
}

// GLOG parser ----------------------------------------------------------------------

fn line_to_log_entry(line: &str) -> LogEntry {
	let mut log_entry = LogEntry { ..Default::default() };
	let mut parser = GlogParserState::PreSection;
	for idx in line.char_indices() {
		parser = match parser {
			GlogParserState::PreSection => {
				if idx.1 == GLOG_SECTION_BEGIN {
					GlogParserState::SectionKind(idx.0 + GLOG_SECTION_BEGIN_SZ)
				} else if idx.1 == GLOG_NEWLINE_R || idx.1 == GLOG_NEWLINE_N {
					GlogParserState::PreSection
				} else {
					GlogParserState::Invalid
				}
			},
			GlogParserState::SectionKind(kind_offset) => {
				if idx.1 == GLOG_INNER_DELIM {
					let kind_str = &line[kind_offset..idx.0];
					//println!("{}", &kind_str);
					let kind = match kind_str {
						"tq" => GlogSectionKind::TimestampMs,
						"s" => GlogSectionKind::Severity,
						"i" => GlogSectionKind::LogSource,
						"m" => GlogSectionKind::Message,
						_ => GlogSectionKind::Unknown, //TODO: Notify of invalid sections?
					};
					GlogParserState::SectionValue(kind, idx.0 + GLOG_INNER_DELIM_SZ)
				} else {
					GlogParserState::SectionKind(kind_offset)
				}
			},
			GlogParserState::SectionValue(kind, value_offset) => {
				if idx.1 == GLOG_SECTION_END {
					GlogParserState::SectionValuePost(kind, value_offset)
				} else {
					GlogParserState::SectionValue(kind, value_offset)
				}
			},
			GlogParserState::SectionValuePost(kind, value_offset) => {
				if idx.1 == GLOG_SECTION_DELIM || idx.1 == GLOG_NEWLINE_R || idx.1 == GLOG_NEWLINE_N {
					if idx.1 == GLOG_SECTION_DELIM && kind == GlogSectionKind::Message {
						//Message is always last - ignore "]:"
						GlogParserState::SectionValue(kind, value_offset)
					} else {
						//Add field to log entry
						let value_str = &line[value_offset..idx.0-GLOG_SECTION_END_SZ];
						//println!("Kind: {:?}, Value: {}", kind, value_str);
						match kind {
							GlogSectionKind::TimestampMs => {
								if let Ok(ts_milli) = value_str.parse::<u64>() {
									let ts_sec   : u64 = ts_milli / 1000;
									let ts_nano  : u32 = ((ts_milli - ts_sec * 1000) * 1000_000) as u32;
									if let Some(ndt) = NaiveDateTime::from_timestamp_opt(ts_sec as i64, ts_nano) {
										log_entry.timestamp = DateTime::<Utc>::from_utc(ndt, Utc);
									} else {
										//TODO: Notify of invalid datetime?
										println!("MALFORMED Log datetime: {}", value_str);
									}
								} else {
									//TODO: Notify of invalid timestamp?
									println!("MALFORMED Log timestamp: {}", value_str);
								}
							},
							GlogSectionKind::Severity => {
								if let Ok(glog_sev_u32) = value_str.parse::<u32>() {
									if let Some(glog_sev) = GlogSeverity::from_u32(glog_sev_u32) {
										log_entry.severity = normalize_glog_sev(glog_sev);
									}
								}
							},
							GlogSectionKind::Message => {
								log_entry.message = value_str.to_string();
							},
							_ => (),
						}
						GlogParserState::PreSection
					}
				} else if idx.1 == GLOG_SECTION_END {
					GlogParserState::SectionValuePost(kind, value_offset)
				} else {
					GlogParserState::SectionValue(kind, value_offset)
				}
			},
			GlogParserState::Invalid => {
				//TODO: Notify of invalid lines?
				println!("MALFORMED Log line: {}", line);
				break;
			},
		};
	}
	log_entry
}

fn normalize_glog_sev(glog_sev : GlogSeverity) -> LogLevel {
	return match glog_sev {
		GlogSeverity::Critical => LogLevel::Critical,
		GlogSeverity::Hardware => LogLevel::Critical,
		GlogSeverity::Error    => LogLevel::Error,
		GlogSeverity::Warning  => LogLevel::Warning,
		GlogSeverity::Info     => LogLevel::Info,
		GlogSeverity::None     => LogLevel::Critical,
	}
}

const GLOG_SECTION_BEGIN : char = '[';
const GLOG_SECTION_BEGIN_SZ : usize = 1; //GLOG_SECTION_BEGIN.len_utf8();
const GLOG_INNER_DELIM : char = '|';
const GLOG_INNER_DELIM_SZ : usize = 1; //GLOG_INNER_DELIM.len_utf8();
const GLOG_SECTION_END : char = ']';
const GLOG_NEWLINE_R : char = '\r';
const GLOG_NEWLINE_N : char = '\n';
const GLOG_SECTION_END_SZ : usize = 1; //GLOG_SECTION_END.len_utf8();
const GLOG_SECTION_DELIM : char = ':';
//const GLOG_SECTION_DELIM_SZ : usize = 1; //GLOG_SECTION_DELIM.len_utf8();

enum GlogSeverity
{
	Critical = 0,
	Hardware = 1,
	Error    = 2,
	Warning  = 3,
	Info     = 4,
	None     = 5,
}
impl GlogSeverity {
	fn from_u32(value: u32) -> Option<GlogSeverity> {
		match value {
			0 => Some(GlogSeverity::Critical),
			1 => Some(GlogSeverity::Hardware),
			2 => Some(GlogSeverity::Error),
			3 => Some(GlogSeverity::Warning),
			4 => Some(GlogSeverity::Info),
			5 => Some(GlogSeverity::None),
			_ => None
		}
	}
}

#[derive(Debug, PartialEq)]
enum GlogSectionKind {
	TimestampMs,
	Severity,
	LogSource,
	Message,
	Unknown,
}

enum GlogParserState {
	PreSection,                               //expect '[', ignore '\r' or '\n'
	SectionKind(usize),                       //expect kind until '|' (kind may not contain '|')
	SectionValue(GlogSectionKind, usize),     //expect value until ']'
	SectionValuePost(GlogSectionKind, usize), //expect ':' or '\r' or '\n', process line
	Invalid,                                  //park in this state on parser error
}

// -------------------------------------------------------------------------------------------------------------

enum LogSourcesColumns {
    Active = 0,
	Inconsistent = 1,
	Text = 2,
}

enum LogEntriesColumns {
    Timestamp = 0,
	Severity = 1,
	Message = 2,
}

fn fixed_toggled_sorted<W: IsA<gtk::CellRendererToggle>>(
	tree_store: &gtk::TreeStore,
    model_sort: &gtk::TreeModelSort,
    _w: &W,
    path: gtk::TreePath,
) {
	fixed_toggled(
		tree_store,
		_w,
		model_sort.convert_path_to_child_path(&path).unwrap());
}

fn fixed_toggled<W: IsA<gtk::CellRendererToggle>>(
    tree_store: &gtk::TreeStore,
    _w: &W,
    path: gtk::TreePath,
) {
	//println!("Path: {:?}", path.get_indices_with_depth());
	
    let iter = tree_store.get_iter(&path).unwrap();
    let mut active = tree_store
        .get_value(&iter, LogSourcesColumns::Active as i32)
        .get::<bool>()
        .unwrap();
	let mut inconsistent = tree_store
        .get_value(&iter, LogSourcesColumns::Inconsistent as i32)
        .get::<bool>()
        .unwrap();
	
	
	if inconsistent || !active {
		inconsistent = false;
		active = true;
	} else {
		active = false;
	}
	
    tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
	tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &inconsistent.to_value());
	
	let mut level_inconsistent = false;
	
	let mut path_forward = path.clone();
	loop {
		path_forward.next();
		if let Some(iter) = tree_store.get_iter(&path_forward) {
			let n_active = tree_store
				.get_value(&iter, LogSourcesColumns::Active as i32)
				.get::<bool>()
				.unwrap();
			
			let n_inconsistent = tree_store
				.get_value(&iter, LogSourcesColumns::Inconsistent as i32)
				.get::<bool>()
				.unwrap();
			
			if n_active != active || n_inconsistent {
				level_inconsistent = true;
				break;
			}
		}
		else {
			break;
		}
	}
	
	let mut path_backwards = path.clone();
	loop {
		if path_backwards.prev() {
			let iter = tree_store.get_iter(&path_backwards).unwrap();
			let n_active = tree_store
				.get_value(&iter, LogSourcesColumns::Active as i32)
				.get::<bool>()
				.unwrap();
			
			let n_inconsistent = tree_store
				.get_value(&iter, LogSourcesColumns::Inconsistent as i32)
				.get::<bool>()
				.unwrap();
			
			if n_active != active || n_inconsistent {
				level_inconsistent = true;
				break;
			}
		}
		else {
			break;
		}
	}
	
	let mut path_up = path.clone();
	loop {
		if path_up.up() && path_up.get_depth() > 0 {
			let iter = tree_store.get_iter(&path_up).unwrap();
			if level_inconsistent {
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &false.to_value());
			}
			else
			{
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
			}
			tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &level_inconsistent.to_value());
		}
		else {
			break;
		}
	}
	
	fn activate_children(tree_store: &gtk::TreeStore, mut path: gtk::TreePath, active : bool) {
		path.down();
		loop {
			if let Some(iter) = tree_store.get_iter(&path)
			{
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
				tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &false.to_value());
				activate_children(tree_store, path.clone(), active);
				path.next();
			}
			else
			{
				break;
			}
		}
	}
	activate_children(tree_store, path, active);
	
	//println!("Inconsistent: {}", level_inconsistent);
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);
	//window.set_icon_from_file("../images/sherlog_icon.png");
	
	let file = File::open("../logfiles/example.glog").expect("Could not open file");
    let mut reader = BufReader::new(file);

	let mut log_entries = Vec::new();
	let mut buf = Vec::<u8>::new();
    while reader.read_until(b'\n', &mut buf).expect("read_until failed") != 0 {
		match String::from_utf8_lossy(&buf) {
			std::borrow::Cow::Borrowed(line_str) => {
				//println!("{}", line_str);
				let curr_entry = line_to_log_entry(&line_str);
				log_entries.push(curr_entry);
				buf.clear();
			},
			std::borrow::Cow::Owned(line_str) => {
				line_to_log_entry(&line_str);
				//TODO: Notify of invalid lines?
				println!("MALFORMED UTF-8: {}", line_str);
			},
		}
    }
	
	//Vec::<LogEntry>::new()
	let log_source_ex = LogSource {name: "example".to_string(), children: {LogSourceContents::Entries(log_entries) } };
	let log_source_ex2 = LogSource {name: "example2".to_string(), children: {LogSourceContents::Entries(Vec::<LogEntry>::new()) } };
	let log_source_ex3 = LogSource {name: "example3".to_string(), children: {LogSourceContents::Entries(Vec::<LogEntry>::new()) } };
	let log_source_ex4_1 = LogSource {name: "example4_1".to_string(), children: {LogSourceContents::Entries(Vec::<LogEntry>::new()) } };
	let log_source_ex4_2 = LogSource {name: "example4_2".to_string(), children: {LogSourceContents::Entries(Vec::<LogEntry>::new()) } };
	let log_source_ex4 = LogSource {name: "examale4".to_string(), children: {LogSourceContents::Sources(vec![log_source_ex4_1, log_source_ex4_2]) } };
	
	let log_source_root = LogSource {name: "Root LogSource".to_string(), children: {LogSourceContents::Sources(
	vec![log_source_ex, log_source_ex2, log_source_ex3, log_source_ex4]) } };

    window.set_title("Sherlog");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(600, 400);
	
	// left pane
    let left_store = TreeStore::new(&[gtk::Type::Bool, gtk::Type::Bool, String::static_type()]);
	let left_store_sort = gtk::TreeModelSort::new(&left_store);
	let left_tree = gtk::TreeView::new_with_model(&left_store_sort);
    left_tree.set_headers_visible(true);
	
	// Column for fixed toggles
    {
		let column = gtk::TreeViewColumn::new();
		// https://lazka.github.io/pgi-docs/Gtk-3.0/classes/TreeViewColumn.html#Gtk.TreeViewColumn.set_sort_indicator
		column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Log source");
		column.set_fixed_width(300);
		column.set_sort_indicator(true);
		column.set_clickable(true);
		column.set_sort_column_id(LogSourcesColumns::Text as i32);
		
		{
			let renderer_toggle = gtk::CellRendererToggle::new();
			//renderer_toggle.set_property_inconsistent(true);
			renderer_toggle.set_alignment(0.0, 0.0);
			//renderer_toggle.set_padding(0, 0);
			let store_clone = left_store.clone();
			let model_sort_clone = left_store_sort.clone();
			renderer_toggle.connect_toggled(move |w, path| fixed_toggled_sorted(&store_clone, &model_sort_clone, w, path));
			column.pack_start(&renderer_toggle, false);
			column.add_attribute(&renderer_toggle, "active", LogSourcesColumns::Active as i32);
			column.add_attribute(&renderer_toggle, "inconsistent", LogSourcesColumns::Inconsistent as i32);
		}
		
		{
			let renderer_text = CellRendererText::new();
			renderer_text.set_alignment(0.0, 0.0);
			column.pack_start(&renderer_text, false);
			column.add_attribute(&renderer_text, "text", LogSourcesColumns::Text as i32);
		}
		left_tree.append_column(&column);
	}
	
	fn build_left_store(store: &TreeStore, log_source: &LogSource, parent: Option<&gtk::TreeIter>) {
		let new_parent = store.insert_with_values(parent, None, &[LogSourcesColumns::Active as u32, LogSourcesColumns::Inconsistent as u32, LogSourcesColumns::Text as u32], &[&false, &false, &log_source.name]);
		match &log_source.children {
			LogSourceContents::Sources(v) => {
				for source in v {
					build_left_store(store, source, Some(&new_parent));
				}
			},
			LogSourceContents::Entries(_v) => {
				()
			},
		}
	}
	build_left_store(&left_store, &log_source_root, None);
	left_tree.expand_all();
	
	let split_pane = gtk::Box::new(Orientation::Horizontal, 10);

    split_pane.set_size_request(-1, -1);
	let scrolled_window = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
	scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	//scrolled_window.set_property("min-content-width", &200);
	scrolled_window.add(&left_tree);
	split_pane.pack_start(&scrolled_window, false, false, 0);
	//https://developer.gnome.org/gtk3/stable/GtkPaned.html
	
	let scrolled_window = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
	scrolled_window.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
	//scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	
	
	//Right side:
	let right_store = ListStore::new(&[String::static_type(), String::static_type(), String::static_type()]);
	let right_store_sort = gtk::TreeModelSort::new(&right_store);
	let right_tree = gtk::TreeView::new_with_model(&right_store_sort);
    right_tree.set_headers_visible(true);
	
	fn build_right_store(store: &ListStore, log_source: &LogSource) {
		match &log_source.children {
			LogSourceContents::Sources(v) => {
				for source in v {
					build_right_store(store, source);
				}
			},
			LogSourceContents::Entries(v) => {
				for entry in v {
					let date_str = entry.timestamp.format("%F %T%.3f").to_string();
					//let date_str = entry.timestamp.to_rfc3339_opts(SecondsFormat::Millis, false);
					store.insert_with_values(None, 
					&[LogEntriesColumns::Timestamp as u32,
					LogEntriesColumns::Severity as u32,
					LogEntriesColumns::Message as u32],
					&[&date_str,
					&entry.severity.to_string(),
					&entry.message
					]);
				}
				()
			},
		}
	}

	{
		let column = gtk::TreeViewColumn::new();
		column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Timestamp");
		column.set_sort_indicator(true);
		column.set_clickable(true);
		column.set_sort_column_id(LogEntriesColumns::Timestamp as i32);
		//column.set_resizable(true);
		//column.set_reorderable(true);
		let renderer_text = CellRendererText::new();
		renderer_text.set_alignment(0.0, 0.0);
		column.pack_start(&renderer_text, false);
		column.add_attribute(&renderer_text, "text", LogEntriesColumns::Timestamp as i32);
		right_tree.append_column(&column);
	}
	
	{
		let column = gtk::TreeViewColumn::new();
		column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Severity");
		column.set_sort_indicator(true);
		column.set_clickable(true);
		column.set_sort_column_id(LogEntriesColumns::Severity as i32);
		//column.set_resizable(true);
		//column.set_reorderable(true);
		let renderer_text = CellRendererText::new();
		renderer_text.set_alignment(0.0, 0.0);
		column.pack_start(&renderer_text, false);
		column.add_attribute(&renderer_text, "text", LogEntriesColumns::Severity as i32);
		
		//column.set_cell_data_func(renderer_text, None);
		
		right_tree.append_column(&column);
	}
	
	{
		let column = gtk::TreeViewColumn::new();
		column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Message");
		column.set_sort_indicator(true);
		column.set_clickable(true);
		column.set_sort_column_id(LogEntriesColumns::Message as i32);
		//column.set_resizable(true);
		//column.set_reorderable(true);
		let renderer_text = CellRendererText::new();
		renderer_text.set_alignment(0.0, 0.0);
		column.pack_start(&renderer_text, false);
		column.add_attribute(&renderer_text, "text", LogEntriesColumns::Message as i32);
		right_tree.append_column(&column);
	}
	
	build_right_store(&right_store, &log_source_root);
	
	scrolled_window.add(&right_tree);
	split_pane.pack_start(&scrolled_window, true, true, 10);

    window.add(&split_pane);
    window.show_all();
}

fn main() {
    let application =
        gtk::Application::new(Some("com.github.BenjaminRi.Sherlog"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&args().collect::<Vec<_>>());
}

/*
DateTime utilities:
let dt = Utc.ymd(2018, 1, 26).and_hms_micro(18, 30, 9, 453_829);
println!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));
let ts_milli : u64 = 1568208334469;
let ts_sec   : u64 = ts_milli / 1000;
let ts_nano  : u32 = ((ts_milli - ts_sec * 1000) * 1000_000) as u32;
let dt = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp_opt(ts_sec as i64, ts_nano).expect("Invalid timestamp encounteres"), Utc);
println!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));
let dt : DateTime::<Utc> = DateTime::<FixedOffset>::parse_from_rfc3339("1996-12-19T16:39:57-08:00").expect("Parse error!").with_timezone(&Utc);
println!("{}", dt.to_rfc3339_opts(SecondsFormat::Millis, false));*/

/*

enum Severity
{
  OFF = SLOG2_SHUTDOWN - 1,    ///filter setting only: no log passes.
  SHUTDOWN = SLOG2_SHUTDOWN,   ///sporadic abnormal event, whole system corrupted, e.g.  (most severe)
  CRITICAL = SLOG2_CRITICAL,   ///sporadic abnormal event, functionality corrupted, e.g.
  ERROR = SLOG2_ERROR,         ///sporadic abnormal event, functionality immediately affected, e.g.
  WARNING = SLOG2_WARNING,     ///sporadic abnormal event, functionality not immediately affected, e.g.
  NOTICE = SLOG2_NOTICE,       ///sporadic data, business content, e.g.
  INFO = SLOG2_INFO,           ///sporadic data, generic content, e.g.
  DEBUG_NORMAL = SLOG2_DEBUG1, ///low bandwidth debug data, business content, e.g.
  DEBUG_DETAIL = SLOG2_DEBUG2, ///high bandwidth debug data, generic content, e.g. all communication accesses (least severe)
  DEFAULT,                     ///filter setting only: set the initial severity.
};
*/
