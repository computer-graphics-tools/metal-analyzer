use super::*;

fn test_doc(text: &str) -> Document {
    Document::new(Url::parse("file:///test.metal").unwrap(), text.to_string(), 1)
}

#[test]
fn line_offsets_empty() {
    let doc = test_doc("");
    assert_eq!(doc.line_count(), 1);
    assert_eq!(doc.line_text(0), Some(""));
}

#[test]
fn line_offsets_basic() {
    let doc = test_doc("hello\nworld\n");
    assert_eq!(doc.line_count(), 3);
    assert_eq!(doc.line_text(0), Some("hello"));
    assert_eq!(doc.line_text(1), Some("world"));
    assert_eq!(doc.line_text(2), Some(""));
}

#[test]
fn offset_roundtrip() {
    let doc = test_doc("float4 pos;\nhalf3 col;\n");
    let pos = Position {
        line: 1,
        character: 0,
    };
    let off = doc.offset_of(pos).unwrap();
    assert_eq!(off, 12); // byte offset of second line
    assert_eq!(doc.position_of(off), pos);
}

#[test]
fn word_at_position() {
    let doc = test_doc("float4 position;");
    let (word, _range) = doc
        .word_at(Position {
            line: 0,
            character: 9,
        })
        .unwrap();
    assert_eq!(word, "position");
}

#[test]
fn set_content_updates_lines() {
    let mut doc = test_doc("one\ntwo");
    assert_eq!(doc.line_count(), 2);
    doc.set_content("a\nb\nc\n".to_string(), 2);
    assert_eq!(doc.line_count(), 4);
    assert_eq!(doc.version, 2);
}

#[test]
fn incremental_change() {
    let mut doc = test_doc("hello world");
    doc.apply_changes(
        vec![TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position {
                    line: 0,
                    character: 6,
                },
                end: Position {
                    line: 0,
                    character: 11,
                },
            }),
            range_length: None,
            text: "Metal".to_string(),
        }],
        2,
    );
    assert_eq!(doc.text, "hello Metal");
    assert_eq!(doc.version, 2);
}

#[test]
fn full_content_change() {
    let mut doc = test_doc("old content");
    doc.apply_changes(
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "new content".to_string(),
        }],
        3,
    );
    assert_eq!(doc.text, "new content");
    assert_eq!(doc.version, 3);
}
