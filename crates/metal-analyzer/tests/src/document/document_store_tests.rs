use super::*;

#[test]
fn store_open_get_content_close() {
    let store = DocumentStore::new();
    let uri = Url::parse("file:///shader.metal").unwrap();
    store.open(uri.clone(), "kernel void f() {}".to_string(), 1);

    assert_eq!(store.get_content(&uri), Some("kernel void f() {}".to_string()));

    store.close(&uri);
    assert!(store.get_content(&uri).is_none());
}

#[test]
fn store_update_existing() {
    let store = DocumentStore::new();
    let uri = Url::parse("file:///shader.metal").unwrap();
    store.open(uri.clone(), "v1".to_string(), 1);
    store.update(uri.clone(), "v2".to_string(), 2);
    let doc = store.get(&uri).unwrap();
    assert_eq!(doc.text, "v2");
    assert_eq!(doc.version, 2);
}

#[test]
fn store_update_unknown_creates() {
    let store = DocumentStore::new();
    let uri = Url::parse("file:///new.metal").unwrap();
    store.update(uri.clone(), "content".to_string(), 1);
    assert!(store.get_content(&uri).is_some());
}

#[test]
fn store_matches_differently_encoded_uris_for_same_file() {
    let store = DocumentStore::new();
    let path_uri = Url::from_file_path(std::env::temp_dir().join("my shaders").join("@scope").join("a.metal")).unwrap();
    let mut client_spelling = path_uri.as_str().replace('@', "%40");
    if let Some(drive) = path_uri.path().get(1..3).filter(|drive| drive.ends_with(':')) {
        client_spelling = client_spelling.replacen(drive, &format!("{}%3A", drive[..1].to_ascii_lowercase()), 1);
    }
    let client_uri = Url::parse(&client_spelling).unwrap();

    store.open(client_uri.clone(), "kernel void f() {}".to_string(), 1);

    assert_eq!(store.get_content(&path_uri), Some("kernel void f() {}".to_string()));
    assert_eq!(store.client_uri(&path_uri), Some(client_uri.clone()));
    assert_eq!(store.all_uris(), vec![client_uri]);

    store.close(&path_uri);
    assert!(store.get_content(&path_uri).is_none());
}
