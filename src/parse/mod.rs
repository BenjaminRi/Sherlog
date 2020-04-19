pub mod glog;
pub mod io; //Central hub for log parser io
pub mod sfile;
pub mod xlog;

pub use self::io::from_file;
