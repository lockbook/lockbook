//! Links between files, across the whole tree: what each note links to and
//! what links to each file.

mod extract;
mod index;
mod notice;
mod reader;
pub mod upkeep;
mod watch;

pub use extract::{Link, LinkKind, Spans, extract};
pub use index::{Gone, Indexed, LinkIndex};
pub use reader::{Read, Reader};
pub use watch::{Notice, Upkeep};
