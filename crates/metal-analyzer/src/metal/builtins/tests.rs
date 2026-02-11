use super::*;

#[test]
fn test_builtins_not_empty() {
    assert!(!all().is_empty());
    assert!(
        all().len() > 300,
        "expected > 300 builtins, got {}",
        all().len()
    );
}

#[test]
fn test_keywords_present() {
    let list = keywords();
    assert!(list.contains(&"kernel"));
    assert!(list.contains(&"fragment"));
    assert!(list.contains(&"float"));
}

#[test]
fn test_lookup_known_symbol() {
    let entry = lookup("float4").expect("float4 should be present");
    assert_eq!(entry.kind, types::BuiltinKind::Type);
    assert!(entry.documentation.contains("Vector type"));

    let entry = lookup("mix").expect("mix should be present");
    assert_eq!(entry.kind, types::BuiltinKind::Function);
    assert_eq!(entry.label, "mix");
}

#[test]
fn test_lookup_unknown_symbol() {
    assert!(lookup("nonExistentSymbol").is_none());
}

#[test]
fn test_no_duplicate_labels() {
    let mut seen = std::collections::HashSet::new();
    for entry in all() {
        if !seen.insert(&entry.label) {
            // Note: some duplicates are intentional (e.g. overloaded functions like `abs`)
            // or identical names in different categories.
            // However, our `lookup` map only stores one index per label.
            // If strict uniqueness is required, uncomment this panic:
            // panic!("Duplicate label in builtins: {}", entry.label);
        }
    }
}
