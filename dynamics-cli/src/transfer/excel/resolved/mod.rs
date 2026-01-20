//! Excel I/O for resolved records (Excel 2)

mod reader;
mod writer;

pub use reader::read_resolved_excel;
pub use writer::write_resolved_excel;
