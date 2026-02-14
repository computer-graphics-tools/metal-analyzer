use super::prefixed_progress_title;

#[test]
fn progress_title_adds_prefix_when_missing() {
    assert_eq!(prefixed_progress_title("Indexing"), "metal-analyzer: Indexing".to_string());
}

#[test]
fn progress_title_preserves_existing_prefix() {
    assert_eq!(prefixed_progress_title("metal-analyzer: Indexing"), "metal-analyzer: Indexing".to_string());
}
