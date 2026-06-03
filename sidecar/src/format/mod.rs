pub mod catalog;
pub mod header;
pub mod payload;
pub mod value;

pub use catalog::Catalog;
pub use header::{Header, FORMAT_VERSION, MAGIC};
pub use payload::PayloadArena;
pub use value::{Value, ValueKind};

/// Maximum byte length for inline string/bytes storage.
pub const INLINE_THRESHOLD: usize = 64;
