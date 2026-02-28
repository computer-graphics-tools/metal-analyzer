pub(crate) mod database;
pub(crate) mod functions;
pub(crate) mod keywords;
pub(crate) mod types;

#[cfg(test)]
#[path = "../../../tests/src/metal/builtins_tests.rs"]
mod tests;

pub use self::{
    database::{all, lookup},
    keywords::KEYWORDS,
    types::{BuiltinEntry, BuiltinKind},
};

pub fn keywords() -> &'static [&'static str] {
    KEYWORDS
}
