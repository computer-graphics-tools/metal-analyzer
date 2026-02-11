mod database;
mod functions;
mod keywords;
mod types;

#[cfg(test)]
mod tests;

pub use self::database::{all, lookup};
pub use self::keywords::KEYWORDS;
pub use self::types::{BuiltinEntry, BuiltinKind};

pub fn keywords() -> &'static [&'static str] {
    KEYWORDS
}
