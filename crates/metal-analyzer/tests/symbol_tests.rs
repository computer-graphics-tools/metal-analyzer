use metal_analyzer::symbols::SymbolProvider;
use tower_lsp::lsp_types::SymbolKind;

#[test]
fn extract_kernel_function() {
    let src = r#"
kernel void myKernel(device float* data [[buffer(0)]]) {
    // ...
}
"#;
    let provider = SymbolProvider::new();
    let symbols = provider.extract_symbols(src);
    assert!(symbols.iter().any(|s| s.name == "myKernel" && s.kind == SymbolKind::FUNCTION));
}

#[test]
fn extract_struct() {
    let src = "struct VertexOut { float4 position; };";
    let provider = SymbolProvider::new();
    let symbols = provider.extract_symbols(src);
    assert!(symbols.iter().any(|s| s.name == "VertexOut" && s.kind == SymbolKind::STRUCT));
}

#[test]
fn quick_definition_finds_function() {
    let src = r#"
void helper(int x) { }
int main() { helper(42); }
"#;
    let provider = SymbolProvider::new();
    let range = provider.quick_definition(src, "helper");
    assert!(range.is_some());
}

#[test]
fn scanner_does_not_extract_parameter_types_as_symbols() {
    let src = r#"
kernel void myKernel(
    const constant Foo* bar [[buffer(0)]],
    device float* data [[buffer(1)]],
    uint id [[thread_position_in_grid]]
) {
}
"#;
    let provider = SymbolProvider::new();
    let symbols = provider.extract_symbols(src);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

    // The function itself should be extracted.
    assert!(names.contains(&"myKernel"), "should find myKernel, got: {names:?}");

    // Type names used in parameter declarations should NOT be extracted as symbols.
    assert!(!names.contains(&"Foo"), "should NOT extract parameter type 'Foo' as a symbol, got: {names:?}");
    assert!(!names.contains(&"float"), "should NOT extract 'float' as a symbol, got: {names:?}");
}

#[test]
fn scanner_does_not_extract_parameter_names_as_symbols() {
    let src = r#"
float4 transform(float4 pos, float scale) {
    return pos * scale;
}
"#;
    let provider = SymbolProvider::new();
    let symbols = provider.extract_symbols(src);
    let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();

    assert!(names.contains(&"transform"), "should find transform, got: {names:?}");
    assert!(!names.contains(&"pos"), "should NOT extract parameter name 'pos' as a symbol, got: {names:?}");
    assert!(!names.contains(&"scale"), "should NOT extract parameter name 'scale' as a symbol, got: {names:?}");
}
