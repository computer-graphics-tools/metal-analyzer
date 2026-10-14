use super::*;

#[cfg(windows)]
#[test]
fn temp_dir_expands_short_names() {
    let dir = temp_dir();
    assert!(dir.is_dir(), "temp dir should exist: {}", dir.display());
    assert!(!dir.to_string_lossy().contains('~'), "expected long folder names, got {}", dir.display());
}

#[cfg(not(windows))]
#[test]
fn temp_dir_matches_std_temp_dir() {
    assert_eq!(temp_dir(), std::env::temp_dir());
}
