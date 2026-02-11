mod database;
mod functions;
mod keywords;
mod types;

#[cfg(test)]
#[path = "../../../tests/src/metal/builtins_tests.rs"]
mod tests;

pub use self::database::{all, lookup};
pub use self::keywords::KEYWORDS;
pub use self::types::{BuiltinEntry, BuiltinKind};

pub fn keywords() -> &'static [&'static str] {
    KEYWORDS
}
