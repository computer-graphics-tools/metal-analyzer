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
