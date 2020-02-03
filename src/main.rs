extern crate gio;
extern crate gtk;
extern crate gdk;
extern crate glib;
extern crate log;
extern crate chrono;
extern crate cairo;

use cairo::{Context, Format, ImageSurface, Rectangle};

use gio::prelude::*;
use gtk::prelude::*;
use gtk::DrawingArea;
use gdk::EventMask;
//use std::cmp::Ordering;

use std::rc::Rc;
use std::cell::RefCell;

mod model;
mod parse;

mod tree_model;

#[allow(unused_imports)]
use gtk::{
    ApplicationWindow, ButtonsType, CellRendererPixbuf, CellRendererText, DialogFlags,
    MessageDialog, MessageType, Orientation, TreeStore, ListStore, TreeView, TreeViewColumn, WindowPosition,
};

use std::env::args;

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
	Id = 3,
	ChildCount = 4,
}

enum LogEntriesColumns {
    Timestamp = 0,
	Severity = 1,
	Message = 2,
	Visible = 3,
	SourceId = 4,
}

fn fixed_toggled_sorted<W: IsA<gtk::CellRendererToggle>>(
	tree_store: &gtk::TreeStore,
	list_store: &gtk::ListStore,
    model_sort: &gtk::TreeModelSort,
    _w: &W,
    path: gtk::TreePath,
) {
	fixed_toggled(
		tree_store,
		list_store,
		_w,
		model_sort.convert_path_to_child_path(&path).unwrap());
}

fn fixed_toggled<W: IsA<gtk::CellRendererToggle>>(
    tree_store: &gtk::TreeStore,
	list_store: &gtk::ListStore,
    _w: &W,
    path: gtk::TreePath,
) {
	//println!("Path: {:?}", path.get_indices_with_depth());
	
    let iter = tree_store.get_iter(&path).unwrap();
    let mut active = tree_store
        .get_value(&iter, LogSourcesColumns::Active as i32)
        .get_some::<bool>()
        .unwrap();
	let mut inconsistent = tree_store
        .get_value(&iter, LogSourcesColumns::Inconsistent as i32)
        .get_some::<bool>()
        .unwrap();
	
	
	if inconsistent || !active {
		inconsistent = false;
		active = true;
	} else {
		active = false;
	}
	
    tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
	tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &inconsistent.to_value());
	
	fn check_inconsistent(tree_store: &gtk::TreeStore, mut path: gtk::TreePath) -> bool{
		let mut prev_active = None;
		if path.up() {
			path.append_index(0);
			while let Some(iter) = tree_store.get_iter(&path) {
				let n_active = tree_store
					.get_value(&iter, LogSourcesColumns::Active as i32)
					.get_some::<bool>()
					.unwrap();
				
				let n_inconsistent = tree_store
					.get_value(&iter, LogSourcesColumns::Inconsistent as i32)
					.get_some::<bool>()
					.unwrap();
				
				if (prev_active != None && Some(n_active) != prev_active) || n_inconsistent {
					return true;
				}
				prev_active = Some(n_active);
				path.next();
			}
		}
		false
	}
	
	{
		let mut path_up = path.clone();
		let mut level_inconsistent = check_inconsistent(tree_store, path_up.clone());
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
			level_inconsistent = check_inconsistent(tree_store, path_up.clone());
		}
	}
	
	fn activate_children(tree_store: &gtk::TreeStore, mut path: gtk::TreePath, active : bool, sources: &mut Vec::<u32>) {
		path.down();
		while let Some(iter) = tree_store.get_iter(&path)
		{
			let n_active = tree_store
					.get_value(&iter, LogSourcesColumns::Active as i32)
					.get_some::<bool>()
					.unwrap();
			if n_active != active {
				let n_id = tree_store
					.get_value(&iter, LogSourcesColumns::Id as i32)
					.get_some::<u32>()
					.unwrap();
				sources.push(n_id);
				tree_store.set_value(&iter, LogSourcesColumns::Active as u32, &active.to_value());
			}
			tree_store.set_value(&iter, LogSourcesColumns::Inconsistent as u32, &false.to_value());
			activate_children(tree_store, path.clone(), active, sources);
			path.next();
		}
	}
	let mut sources = Vec::<u32>::new();
	let id = tree_store
		.get_value(&iter, LogSourcesColumns::Id as i32)
		.get_some::<u32>()
		.unwrap();
	sources.push(id);
	activate_children(tree_store, path, active, &mut sources);
	println!("Click: {:?} change to {}", sources, active);
	
	{
		let mut path = gtk::TreePath::new_from_indicesv(&[0]);
		while let Some(iter) = list_store.get_iter(&path) {
			let id = list_store
				.get_value(&iter, LogEntriesColumns::SourceId as i32)
				.get_some::<u32>()
				.unwrap();
			if sources.contains(&id) {
				list_store.set_value(&iter, LogEntriesColumns::Visible as u32, &active.to_value());
			}
			path.next();
		}
	}
	
	//println!("Inconsistent: {}", level_inconsistent);
}

fn build_ui(application: &gtk::Application, file_paths: &[std::path::PathBuf]) {
    let window = gtk::ApplicationWindow::new(application);
	//window.set_icon_from_file("../images/sherlog_icon.png");
    window.set_title("Sherlog");
    window.set_border_width(10);
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(600, 400);
	
	//let args = &args().collect::<Vec<_>>();
    /*//FIXME: Use handle-local-options once https://github.com/gtk-rs/gtk/issues/580 is a thing
    let mut new_instance = false;
    for arg in args {
        match arg.as_str() {
            "-n" | "--new-instance" => new_instance = true,
            _ => (),
        }
    }*/
    //println!("{:?}", args);
	
	//Generate fake log entries to test GUI ---------------------------------------------
	
	let log_entries = vec![
		model::LogEntry { message: "TestCritical 121343245345".to_string(), severity: model::LogLevel::Critical, ..Default::default() },
		model::LogEntry { message: "TestError 3405834068".to_string(),      severity: model::LogLevel::Error,    ..Default::default() },
		model::LogEntry { message: "TestWarning 340958349068".to_string(),  severity: model::LogLevel::Warning,  ..Default::default() },
		model::LogEntry { message: "TestInfo 3049580349568".to_string(),    severity: model::LogLevel::Info,     ..Default::default() },
		model::LogEntry { message: "TestDebug 0345986045968".to_string(),   severity: model::LogLevel::Debug,    ..Default::default() },
		model::LogEntry { message: "TestTrace 309468456".to_string(),       severity: model::LogLevel::Trace,    ..Default::default() },
		];
	
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
	
	//---------------------------------------------------------------------------------------
	
	println!("{:?}", file_paths);
	let log_source_root = if !file_paths.is_empty() {
		if file_paths.len() > 1 {
			println!("WARNING: Multiple files opened, ignoring all but the first one.");
		}
		parse::from_file(&file_paths[0]).expect("Could not read file!")
	} else {
		log_source_root
	};
	
	
	let mut log_source_root_ext = extend_log_source(log_source_root);
	log_source_root_ext.generate_ids();
	log_source_root_ext.calc_child_cnt();
	
	// left pane
	let right_store = ListStore::new(&[String::static_type(), String::static_type(), String::static_type(), glib::Type::Bool, glib::Type::U32]);
    let left_store = TreeStore::new(&[glib::Type::Bool, glib::Type::Bool, String::static_type(), glib::Type::U32, glib::Type::U64]);
	let left_store_sort = gtk::TreeModelSort::new(&left_store);
	let left_tree = gtk::TreeView::new_with_model(&left_store_sort);
    left_tree.set_headers_visible(true);
	
	
	//https://github.com/ChariotEngine/drs-studio/blob/f0303b52063f0d365732941e5096c42dad06f326/ui/gtk/src/main.rs
	let store_clone = left_store_sort.clone();
	left_store_sort.set_sort_func(gtk::SortColumn::Index(LogSourcesColumns::Text as u32), move |_w, l_it, r_it| {
		let l_id = store_clone
			.get_value(&l_it, LogSourcesColumns::ChildCount as i32)
			.get_some::<u64>()
			.unwrap();
		let r_id = store_clone
			.get_value(&r_it, LogSourcesColumns::ChildCount as i32)
			.get_some::<u64>()
			.unwrap();
		l_id.cmp(&r_id)
	} );
	
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
			let left_store_clone = left_store.clone(); //GTK objects are refcounted, just clones ref
			let right_store_clone = right_store.clone(); //GTK objects are refcounted, just clones ref
			let model_sort_clone = left_store_sort.clone(); //GTK objects are refcounted, just clones ref
			renderer_toggle.connect_toggled(move |w, path| fixed_toggled_sorted(&left_store_clone, &right_store_clone, &model_sort_clone, w, path));
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
			LogSourcesColumns::Id as u32,
			LogSourcesColumns::ChildCount as u32,
			],
			&[
			&true,
			&false,
			&log_source.name,
			&log_source.id,
			&log_source.child_cnt
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
	scrolled_window_left.set_property("overlay-scrolling", &false).unwrap();
	scrolled_window_left.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	//scrolled_window_left.set_property("min-content-width", &200);
	scrolled_window_left.add(&left_tree);
	split_pane.pack_start(&scrolled_window_left, false, false, 0);
	//https://developer.gnome.org/gtk3/stable/GtkPaned.html
	
	let scrolled_window_right = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
	scrolled_window_right.set_property("overlay-scrolling", &false).unwrap();
	//https://stackoverflow.com/questions/50414957/gtk3-0-scrollbar-on-treeview-scrolledwindow-css-properties-to-control-scrol
	//scrolled_window_right.get_hscrollbar().unwrap().set_property("has-backward-stepper", &true).unwrap();
	//scrolled_window_right.get_vscrollbar().unwrap().set_property("has-backward-stepper", &true).unwrap();
	scrolled_window_right.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
	//scrolled_window_right.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
	
	
	//Right side:
	let right_store_filter = gtk::TreeModelFilter::new(&right_store, None);
	right_store_filter.set_visible_column(LogEntriesColumns::Visible as i32);
	//let right_store_filter_sort = gtk::TreeModelSort::new(&right_store_filter);
	let right_tree = gtk::TreeView::new_with_model(&right_store_filter);//right_store_filter_sort
    right_tree.set_headers_visible(true);
	
	// CRUCIAL for performance, also stops nasty dynamic 
	// line loading which moves the scrollbar on its own:
	right_tree.set_fixed_height_mode(true);
	
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
					LogEntriesColumns::Visible as u32,
					LogEntriesColumns::SourceId as u32
					],
					&[&date_str,
					&entry.severity.to_string(),
					&entry.message,
					&(true), //entry.severity == model::LogLevel::Error
					&log_source.id
					]);
				}
				()
			},
		}
	}
	build_right_store(&right_store, &log_source_root_ext);

	{
		let column = gtk::TreeViewColumn::new();
		column.set_sizing(gtk::TreeViewColumnSizing::Fixed);
		column.set_title("Timestamp");
		//column.set_sort_indicator(true);
		//column.set_clickable(true);
		//column.set_sort_column_id(LogEntriesColumns::Timestamp as i32);
		column.set_resizable(true);
		//column.set_reorderable(true);
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
		//column.set_sort_indicator(true);
		//column.set_clickable(true);
		//column.set_sort_column_id(LogEntriesColumns::Severity as i32);
		column.set_resizable(true);
		//column.set_reorderable(true);
		let renderer_text = CellRendererText::new();
		//renderer_text.set_alignment(0.0, 0.0);
		column.pack_start(&renderer_text, false);
		column.add_attribute(&renderer_text, "text", LogEntriesColumns::Severity as i32);
		
		gtk::TreeViewColumnExt::set_cell_data_func(&column, &renderer_text, Some(Box::new(move |_column, cell, model, iter| {
			//let path = model.get_path(iter);
			let sev = model
				.get_value(&iter, LogEntriesColumns::Severity as i32)
				.get::<String>()
				.unwrap()
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
		//column.set_sort_indicator(true);
		//column.set_clickable(true);
		//column.set_sort_column_id(LogEntriesColumns::Message as i32);
		column.set_resizable(true);
		//column.set_reorderable(true);
		let renderer_text = CellRendererText::new();
		//renderer_text.set_alignment(0.0, 0.0);
		column.pack_start(&renderer_text, false);
		column.add_attribute(&renderer_text, "text", LogEntriesColumns::Message as i32);
		right_tree.append_column(&column);
	}
	
	// Assemble log store ----------------------------------------------------------
	
	let mut store = Vec::<model::LogEntry>::new();
	
	fn build_log_store(store: &mut Vec<model::LogEntry>, log_source: &mut LogSourceExt) {
		match &mut log_source.children {
			LogSourceContentsExt::Sources(v) => {
				for source in v {
					build_log_store(store, source);
				}
			},
			LogSourceContentsExt::Entries(v) => {
				store.append(v);
				()
			},
		}
	}
	
	build_log_store(&mut store, &mut log_source_root_ext);
	
	struct LogStoreLinear {
		store : Vec::<model::LogEntry>,
		cursor_pos : u32,
	}
	
	let store = LogStoreLinear { store : store, cursor_pos : 0 };
	
	//-------------------------------------------------------------------------------
	
	
	fn draw(store: &mut LogStoreLinear, _drawing_area: &DrawingArea, ctx: &cairo::Context) -> gtk::Inhibit {
		//store.store.push(model::LogEntry { message: "TestTrace 309468456".to_string(),       severity: model::LogLevel::Trace,    ..Default::default() });
		//println!("{}", store.store.len());
		
		// crucial for transparency
		/*ctx.set_source_rgba(1.0, 0.0, 0.0, 1.0);
		ctx.set_operator(cairo::Operator::Screen);
		ctx.paint();*/
		
		ctx.set_source_rgb(1.0, 1.0, 1.0);
		ctx.paint();
		
		ctx.set_source_rgb(1.0, 0.0, 0.0);
        ctx.rectangle(10.0, 10.0, 2.0, 2.0);
        ctx.fill();
		
		ctx.set_source_rgb(0.0, 0.0, 0.0);
		ctx.new_sub_path();
        ctx.move_to(10.0, 10.0);
        ctx.line_to(10.0, 20.0);
        ctx.line_to(20.0, 20.0);
        ctx.line_to(20.0, 10.0);
        ctx.close_path();
		ctx.stroke();
		
		ctx.move_to(30.0, 10.0);
		ctx.set_font_size(14.0);
		/*let font_face = ctx.get_font_face();
		let new_font_face = cairo::FontFace::toy_create("cairo :monospace", font_face.toy_get_slant(), font_face.toy_get_weight());
		ctx.set_font_face(&new_font_face);*/
		ctx.select_font_face("Lucida Console", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
		ctx.show_text("Hello canvas!llllllMMMM");
		
		println!("Redraw");
		
		gtk::Inhibit(false)
	}
	
	fn handle_event(store: &mut LogStoreLinear, _drawing_area: &DrawingArea, evt: &gdk::Event) -> gtk::Inhibit {
		//_drawing_area.queue_draw();
		//println!("Event");
		gtk::Inhibit(false)
	}
	
	//scrolled_window_right.add(&right_tree);
	/*let surface = ImageSurface::create(Format::ARgb32, 100, 100)
        .expect("Could not create surface.");
    let ctx = Context::new(&surface);*/
	let drawing_area = DrawingArea::new();
	let event_mask = EventMask::POINTER_MOTION_MASK
		| EventMask::BUTTON_PRESS_MASK | EventMask::BUTTON_RELEASE_MASK
		| EventMask::KEY_PRESS_MASK | EventMask::KEY_RELEASE_MASK | EventMask::SCROLL_MASK;

	drawing_area.set_can_focus(true);
	drawing_area.add_events(event_mask);
	let f = Rc::new(RefCell::new(store));
	let f_clone_1 = f.clone();
	drawing_area.connect_event(move |x, y| handle_event(&mut f_clone_1.clone().borrow_mut(), x, y));

	// establish a reasonable minimum view size
	drawing_area.set_size_request(200, 200);
	let f_clone_2 = f.clone();
	drawing_area.connect_draw(move |x, y| draw(&mut f_clone_2.clone().borrow_mut(), x, y));
	//scrolled_window_right.add(&drawing_area);
	split_pane.pack_start(&drawing_area, true, true, 10);
	
	//https://gtk-rs.org/docs/gtk/prelude/trait.TreeSortableExtManual.html#tymethod.set_sort_func
	println!("SORT FUNC: {}", right_store.has_default_sort_func());
	println!("SORT COLUMN: {:?}", right_store.get_sort_column_id());
	//TODO: If we add the log entries correctly into the tree, we don't even have to do this sort hack any more.
	right_store.set_sort_column_id(gtk::SortColumn::Index(LogEntriesColumns::Timestamp as u32), gtk::SortType::Ascending);
	right_store.set_unsorted();
	println!("SORT COLUMN: {:?}", right_store.get_sort_column_id());

    window.add(&split_pane);
    window.show_all();
}

fn gio_files_to_paths(gio_files : &[gio::File]) -> Vec<std::path::PathBuf> {
	let mut result = Vec::new();
	for gio_file in gio_files {
		result.push(gio_file.get_path().expect("Invalid file path"));
	}
	result
}

fn main() {
	
	// https://developer.gnome.org/CommandLine/
	// https://developer.gnome.org/GtkApplication/
	
    let application =
        gtk::Application::new(Some("com.github.BenjaminRi.Sherlog"), gio::ApplicationFlags::HANDLES_OPEN)
            .expect("Initialization failed...");
	
	// https://gtk-rs.org/docs/glib/struct.OptionFlags.html
	// https://gtk-rs.org/docs/glib/enum.OptionArg.html
	application.add_main_option(
		"CTest",
		glib::Char::new('c').unwrap(),
		glib::OptionFlags::IN_MAIN,
		glib::OptionArg::String,
		"This is just a test",
		Some("This is a test argument"),
	);
	
	// https://gtk-rs.org/docs/gio/prelude/trait.ApplicationExtManual.html
	application.connect_open(move |app, gio_files, _| {
		build_ui(app, &gio_files_to_paths(gio_files));
	});

    application.connect_activate(|app| {
        build_ui(app, &Vec::new());
    });
	
	// https://gtk-rs.org/docs/gio/prelude/trait.ApplicationExtManual.html#tymethod.run
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
