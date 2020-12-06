use super::super::model;

pub fn to_log_entries(_reader: impl std::io::Read, root: model::LogSource) -> model::LogSource {
	root
}

//2020-12-01 15:46:19.122013 (warning) <0x00000001> [] : Foo

//timestamp (severity) <address?> [source?] : message

//(fatal)
//(error)
//(warning)
//(info)
//(debug)
//(trace)
