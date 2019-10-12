extern crate gio;
extern crate gtk;
extern crate log;
extern crate chrono;

use gio::prelude::*;
use gtk::prelude::*;

mod model;
mod parse;

#[allow(unused_imports)]
use gtk::{
    ApplicationWindow, ButtonsType, CellRendererPixbuf, CellRendererText, DialogFlags,
    MessageDialog, MessageType, Orientation, TreeStore, ListStore, TreeView, TreeViewColumn, WindowPosition,
};

use std::env::args;

use std::fs::File;
use std::io::{BufRead, BufReader};

// Extended log source (not part of the API)
enum LogSourceContentsExt {
	Sources(Vec::<LogSourceExt>),
	Entries(Vec::<model::LogEntry>),
}

// Extended log source (not part of the API)
struct LogSourceExt {
	name : String,
	id : u32,
	child_cnt : u64,
	children : LogSourceContentsExt,
}

fn extend_log_source(log_source : model::LogSource) -> LogSourceExt {
	let children = match log_source.children {
		model::LogSourceContents::Sources(v) => {
			let mut contents = Vec::<LogSourceExt>::new();
			contents.reserve(v.len());
			for source in v {
				contents.push(extend_log_source(source));
			}
			LogSourceContentsExt::Sources(contents)
		},
		model::LogSourceContents::Entries(v) => {
			LogSourceContentsExt::Entries(v)
		},
	};
	LogSourceExt {name: log_source.name, id: 0, child_cnt: 0, children: children}
}

impl LogSourceExt {
	fn calc_child_cnt(&mut self) {
		self.child_cnt = match &mut self.children {
			LogSourceContentsExt::Sources(v) => {
				let mut child_cnt : u64 = 0;
				for source in v {
					source.calc_child_cnt();
					child_cnt += source.child_cnt;
				}
				child_cnt
			},
			LogSourceContentsExt::Entries(v) => {
				v.len() as u64
			},
		}
	}
	fn generate_ids(&mut self) -> u32 {
		match &mut self.children {
			LogSourceContentsExt::Sources(v) => {
				let mut id_idx = self.id;
				for source in v {
					id_idx += 1;
					source.id = id_idx;
					id_idx = source.generate_ids();
				}
				id_idx
			},
			LogSourceContentsExt::Entries(_v) => {
				self.id
			},
		}
	}
}

enum LogSourcesColumns {
    Active = 0,
	Inconsistent = 1,
	Text = 2,
	ChildCount = 3,
}

enum LogEntriesColumns {
    Timestamp = 0,
	Severity = 1,
	Message = 2,
	Visible = 3,
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
	

	
	{
		let mut path_forward = path.clone();
		path_forward.next();
		while let Some(iter) = tree_store.get_iter(&path_forward) {
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
			path_forward.next();
		}
	}
	
	{
		let mut path_backwards = path.clone();
		while path_backwards.prev() {
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
	}
	
	{
	let mut path_up = path.clone();
		while path_up.up() && path_up.get_depth() > 0 {
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
	}
	
	fn activate_children(tree_store: &gtk::TreeStore, mut path: gtk::TreePath, active : bool) {
		path.down();
		while let Some(iter) = tree_store.get_iter(&path)
		{
			tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
			tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &false.to_value());
			activate_children(tree_store, path.clone(), active);
			path.next();
		}
	}
	activate_children(tree_store, path, active);
	
	//println!("Inconsistent: {}", level_inconsistent);
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);
	//window.set_icon_from_file("../images/sherlog_icon.png");
    window.set_title("Sherlog");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(600, 400);
	
	let file = File::open("../logfiles/example.glog").expect("Could not open file");
    let mut reader = BufReader::new(file);

	let mut log_entries = Vec::new();
	let mut buf = Vec::<u8>::new();
    while reader.read_until(b'\n', &mut buf).expect("read_until failed") != 0 {
		match String::from_utf8_lossy(&buf) {
			std::borrow::Cow::Borrowed(line_str) => {
				//println!("{}", line_str);
				let curr_entry = parse::glog::line_to_log_entry(&line_str);
				log_entries.push(curr_entry);
				buf.clear();
			},
			std::borrow::Cow::Owned(line_str) => {
				parse::glog::line_to_log_entry(&line_str);
				//TODO: Notify of invalid lines?
				println!("MALFORMED UTF-8: {}", line_str);
			},
		}
    }
	
	//Vec::<model::LogEntry>::new()
	let log_source_ex = model::LogSource {name: "example".to_string(), children: {model::LogSourceContents::Entries(log_entries) } };
	let log_source_ex2_1 = model::LogSource {name: "example2_1".to_string(), children: {model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) } };
	let log_source_ex2 = model::LogSource {name: "example2".to_string(), children: {model::LogSourceContents::Sources(vec![log_source_ex2_1]) } };
	let log_source_ex3 = model::LogSource {name: "example3".to_string(), children: {model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) } };
	let log_source_ex4_1 = model::LogSource {name: "example4_1".to_string(), children: {model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) } };
	let log_source_ex4_2 = model::LogSource {name: "example4_2".to_string(), children: {model::LogSourceContents::Entries(Vec::<model::LogEntry>::new()) } };
	let log_source_ex4 = model::LogSource {name: "examale4".to_string(), children: {model::LogSourceContents::Sources(vec![log_source_ex4_1, log_source_ex4_2]) } };
	
	let log_source_root = model::LogSource {name: "Root LogSource".to_string(), children: {model::LogSourceContents::Sources(
	vec![log_source_ex, log_source_ex2, log_source_ex3, log_source_ex4]) } };
	
	
	let mut log_source_root_ext = extend_log_source(log_source_root);
	log_source_root_ext.generate_ids();
	log_source_root_ext.calc_child_cnt();
	
	// left pane
    let left_store = TreeStore::new(&[gtk::Type::Bool, gtk::Type::Bool, String::static_type(), gtk::Type::U64]);
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
			let store_clone = left_store.clone(); //GTK objects are refcounted, just clones ref
			let model_sort_clone = left_store_sort.clone(); //GTK objects are refcounted, just clones ref
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
	
	{
		let column = gtk::TreeViewColumn::new();
		column.set_title("Entries");
		column.set_sort_indicator(true);
		column.set_clickable(true);
		column.set_sort_column_id(LogSourcesColumns::ChildCount as i32);
		
		{
			let renderer_text = CellRendererText::new();
			renderer_text.set_alignment(0.0, 0.0);
			column.pack_start(&renderer_text, false);
			column.add_attribute(&renderer_text, "text", LogSourcesColumns::ChildCount as i32);
		}
		left_tree.append_column(&column);
	}
	
	fn build_left_store(store: &TreeStore, log_source: &LogSourceExt, parent: Option<&gtk::TreeIter>) {
		let new_parent = store.insert_with_values(
			parent,
			None,
			&[
			LogSourcesColumns::Active as u32,
			LogSourcesColumns::Inconsistent as u32,
			LogSourcesColumns::Text as u32,
			LogSourcesColumns::ChildCount as u32,
			],
			&[
			&false,
			&false,
			&log_source.name,
			&log_source.id
			]);
		match &log_source.children {
			LogSourceContentsExt::Sources(v) => {
				for source in v {
					build_left_store(store, source, Some(&new_parent));
				}
			},
			LogSourceContentsExt::Entries(_v) => {
				()
			},
		}
	}
	build_left_store(&left_store, &log_source_root_ext, None);
	left_tree.expand_all();
	
	let split_pane = gtk::Box::new(Orientation::Horizontal, 10);

    split_pane.set_size_request(-1, -1);
	let scrolled_window_left = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
	scrolled_window_left.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	//scrolled_window_left.set_property("min-content-width", &200);
	scrolled_window_left.add(&left_tree);
	split_pane.pack_start(&scrolled_window_left, false, false, 0);
	//https://developer.gnome.org/gtk3/stable/GtkPaned.html
	
	let scrolled_window_right = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
	scrolled_window_right.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
	//scrolled_window_right.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	
	
	//Right side:
	let right_store = ListStore::new(&[String::static_type(), String::static_type(), String::static_type(), gtk::Type::Bool]);
	let right_store_filter = gtk::TreeModelFilter::new(&right_store, None);
	right_store_filter.set_visible_column(LogEntriesColumns::Visible as i32);
	let left_store_filter_sort = gtk::TreeModelSort::new(&right_store_filter);
	let right_tree = gtk::TreeView::new_with_model(&left_store_filter_sort);
    right_tree.set_headers_visible(true);
	
	fn build_right_store(store: &ListStore, log_source: &LogSourceExt) {
		match &log_source.children {
			LogSourceContentsExt::Sources(v) => {
				for source in v {
					build_right_store(store, source);
				}
			},
			LogSourceContentsExt::Entries(v) => {
				for entry in v {
					let date_str = entry.timestamp.format("%F %T%.3f").to_string();
					//let date_str = entry.timestamp.to_rfc3339_opts(SecondsFormat::Millis, false);
					store.insert_with_values(None, 
					&[LogEntriesColumns::Timestamp as u32,
					LogEntriesColumns::Severity as u32,
					LogEntriesColumns::Message as u32,
					LogEntriesColumns::Visible as u32],
					&[&date_str,
					&entry.severity.to_string(),
					&entry.message,
					&(true) //entry.severity == model::LogLevel::Error
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
		column.set_resizable(true);
		column.set_reorderable(true);
		let renderer_text = CellRendererText::new();
		//renderer_text.set_alignment(0.0, 0.0);
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
		column.set_resizable(true);
		column.set_reorderable(true);
		let renderer_text = CellRendererText::new();
		//renderer_text.set_alignment(0.0, 0.0);
		column.pack_start(&renderer_text, false);
		column.add_attribute(&renderer_text, "text", LogEntriesColumns::Severity as i32);
		
		gtk::TreeViewColumnExt::set_cell_data_func(&column, &renderer_text, Some(Box::new(move |_column, cell, model, iter| {
			//let path = model.get_path(iter);
			let sev = model
				.get_value(&iter, LogEntriesColumns::Severity as i32)
				.get::<String>()
				.unwrap();
			//println!("Severity: {}", sev);
			let color = match sev.as_ref() {
				"Critical" => "#FF0000",
				"Error"    => "#FF0000",
				"Warning"  => "#FFF200",
				_          => "#FFFFFF"
			};
			cell.set_property("cell-background", &color).unwrap();
		})));
		//gtk::CellLayoutExt::set_cell_data_func(&column, &renderer_text, None);
		
		right_tree.append_column(&column);
	}
	
	{
		let column = gtk::TreeViewColumn::new();
		column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Message");
		column.set_sort_indicator(true);
		column.set_clickable(true);
		column.set_sort_column_id(LogEntriesColumns::Message as i32);
		column.set_resizable(true);
		column.set_reorderable(true);
		let renderer_text = CellRendererText::new();
		//renderer_text.set_alignment(0.0, 0.0);
		column.pack_start(&renderer_text, false);
		column.add_attribute(&renderer_text, "text", LogEntriesColumns::Message as i32);
		right_tree.append_column(&column);
	}
	
	build_right_store(&right_store, &log_source_root_ext);
	
	scrolled_window_right.add(&right_tree);
	split_pane.pack_start(&scrolled_window_right, true, true, 10);

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
