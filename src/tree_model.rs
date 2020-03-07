extern crate gobject_sys as gobject;
extern crate gtk;
extern crate gtk_sys;

/*
// https://raw.githubusercontent.com/gtk-rs/sys/master/gtk-sys/src/lib.rs
pub fn foo() -> gtk_sys::GtkTreeModelIface {
	gtk_sys::GtkTreeModelIface {
		g_iface: gobject::GTypeInterface {
			g_type: 0,
			g_instance_type: 0,
		},
		row_changed: None, // Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreePath, *mut GtkTreeIter)>
		row_inserted: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreePath, *mut GtkTreeIter)>,
		row_has_child_toggled: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreePath, *mut GtkTreeIter)>,
		row_deleted: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreePath)>,
		rows_reordered: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreePath, *mut GtkTreeIter, *mut c_int),	>,
		get_flags: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel) -> GtkTreeModelFlags>,
		get_n_columns: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel) -> c_int>,
		get_column_type: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, c_int) -> GType>,
		get_iter: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter, *mut GtkTreePath) -> gboolean, >,
		get_path: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter) -> *mut GtkTreePath>,
		get_value: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter, c_int, *mut gobject::GValue), >,
		iter_next: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter) -> gboolean>,
		iter_previous: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter) -> gboolean>,
		iter_children: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter, *mut GtkTreeIter) -> gboolean, >,
		iter_has_child: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter) -> gboolean>,
		iter_n_children: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter) -> c_int>,
		iter_nth_child: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter, *mut GtkTreeIter, c_int,) -> gboolean, >,
		iter_parent: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter, *mut GtkTreeIter) -> gboolean, >,
		ref_node: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter)>,
		unref_node: None, //Option<unsafe extern "C" fn(*mut GtkTreeModel, *mut GtkTreeIter)>,
	}
}

*/
