mod compress;
mod decompress;

pub use compress::{compress, CompressError};
pub use decompress::{decompress, DecompressError};
