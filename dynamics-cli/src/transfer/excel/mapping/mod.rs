//! Excel I/O for mapping configurations (Excel 1)

mod reader;
mod values;
mod writer;

pub use reader::read_mapping_excel;
pub use writer::write_mapping_excel;
