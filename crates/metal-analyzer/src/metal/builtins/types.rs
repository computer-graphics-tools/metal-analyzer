/// What kind of symbol this builtin represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinKind {
    Keyword,
    Type,
    Function,
    Attribute,
    Snippet,
    Constant,
}

/// A static database entry for a Metal built-in symbol.
#[derive(Debug, Clone)]
pub struct BuiltinEntry {
    pub label: String,
    pub detail: String,
    pub documentation: String,
    pub insert_text: Option<String>,
    pub is_snippet: bool,
    pub kind: BuiltinKind,
    pub category: Option<&'static str>,
}

impl BuiltinEntry {
    pub(crate) fn keyword(label: &str, doc: &str) -> Self {
        Self {
            label: label.to_string(),
            detail: "keyword".to_string(),
            documentation: doc.to_string(),
            insert_text: None,
            is_snippet: false,
            kind: BuiltinKind::Keyword,
            category: None,
        }
    }

    pub(crate) fn typ(label: &str, doc: &str) -> Self {
        Self {
            label: label.to_string(),
            detail: "builtin type".to_string(),
            documentation: doc.to_string(),
            insert_text: None,
            is_snippet: false,
            kind: BuiltinKind::Type,
            category: None,
        }
    }

    pub(crate) fn func(label: &str, detail: &str, doc: &str, cat: &'static str) -> Self {
        Self {
            label: label.to_string(),
            detail: detail.to_string(),
            documentation: doc.to_string(),
            insert_text: None,
            is_snippet: false,
            kind: BuiltinKind::Function,
            category: Some(cat),
        }
    }

    pub(crate) fn attr(label: &str, doc: &str, snippet: Option<&str>) -> Self {
        Self {
            label: label.to_string(),
            detail: "attribute".to_string(),
            documentation: doc.to_string(),
            insert_text: snippet.map(|s| s.to_string()),
            is_snippet: snippet.is_some(),
            kind: BuiltinKind::Attribute,
            category: None,
        }
    }

    pub(crate) fn snippet(label: &str, detail: &str, snippet: &str) -> Self {
        Self {
            label: label.to_string(),
            detail: detail.to_string(),
            documentation: String::new(),
            insert_text: Some(snippet.to_string()),
            is_snippet: true,
            kind: BuiltinKind::Snippet,
            category: None,
        }
    }

    pub(crate) fn constant(label: &str, detail: &str, doc: &str) -> Self {
        Self {
            label: label.to_string(),
            detail: detail.to_string(),
            documentation: doc.to_string(),
            insert_text: None,
            is_snippet: false,
            kind: BuiltinKind::Constant,
            category: None,
        }
    }
}
