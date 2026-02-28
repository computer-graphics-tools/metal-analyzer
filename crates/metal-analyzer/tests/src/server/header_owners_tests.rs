use super::*;

#[test]
fn parse_include_directives_detects_system_and_local() {
    let src = r#"
#include <metal_stdlib>
#include "common/utils.h"
// #include "ignored.h"
"#;
    let parsed = parse_include_directives(src);
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0], ("metal_stdlib".to_owned(), true));
    assert_eq!(parsed[1], ("common/utils.h".to_owned(), false));
}

#[test]
fn update_owner_links_replaces_previous_headers() {
    let headers_to_owners = DashMap::new();
    let owners_to_headers = DashMap::new();
    let owner = PathBuf::from("/tmp/owner.metal");
    let h1 = PathBuf::from("/tmp/a.h");
    let h2 = PathBuf::from("/tmp/b.h");

    update_owner_links(&headers_to_owners, &owners_to_headers, &owner, BTreeSet::from([h1.clone()]));
    update_owner_links(&headers_to_owners, &owners_to_headers, &owner, BTreeSet::from([h2.clone()]));

    assert!(headers_to_owners.get(&h1).is_none());
    assert!(headers_to_owners.get(&h2).is_some());
    assert_eq!(owners_to_headers.get(&owner).expect("owner exists").iter().cloned().collect::<Vec<_>>(), vec![h2]);
}
