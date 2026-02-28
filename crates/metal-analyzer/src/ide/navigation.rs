use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IdePosition {
    pub line: u32,
    pub character: u32,
}

impl IdePosition {
    pub const fn new(
        line: u32,
        character: u32,
    ) -> Self {
        Self {
            line,
            character,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct IdeRange {
    pub start: IdePosition,
    pub end: IdePosition,
}

impl IdeRange {
    pub const fn new(
        start: IdePosition,
        end: IdePosition,
    ) -> Self {
        Self {
            start,
            end,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdeLocation {
    pub file_path: PathBuf,
    pub range: IdeRange,
}

impl IdeLocation {
    pub fn new(
        file_path: impl Into<PathBuf>,
        range: IdeRange,
    ) -> Self {
        Self {
            file_path: file_path.into(),
            range,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationTarget {
    Single(IdeLocation),
    Multiple(Vec<IdeLocation>),
}

impl NavigationTarget {
    pub fn from_locations(locations: Vec<IdeLocation>) -> Option<Self> {
        match locations.len() {
            0 => None,
            1 => locations.first().cloned().map(Self::Single),
            _ => Some(Self::Multiple(locations)),
        }
    }
}
