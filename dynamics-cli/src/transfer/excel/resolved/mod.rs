//! Excel I/O for resolved records (Excel 2)

mod writer;
mod reader;

pub use writer::write_resolved_excel;
pub use reader::read_resolved_excel;
