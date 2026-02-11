use std::collections::HashMap;
use std::sync::OnceLock;

use super::functions;
use super::keywords::KEYWORDS;
use super::types::BuiltinEntry;

static ALL_BUILTINS: OnceLock<Vec<BuiltinEntry>> = OnceLock::new();
static BUILTIN_MAP: OnceLock<HashMap<String, usize>> = OnceLock::new();

fn build_builtins() -> Vec<BuiltinEntry> {
    let mut entries = Vec::with_capacity(1000);

    for &kw in KEYWORDS {
        entries.push(BuiltinEntry::keyword(kw, "Metal keyword"));
    }

    functions::add_scalar_types(&mut entries);
    functions::add_vector_types(&mut entries);
    functions::add_matrix_types(&mut entries);
    functions::add_texture_types(&mut entries);
    functions::add_sampler_types(&mut entries);
    functions::add_atomic_types(&mut entries);
    functions::add_packed_types(&mut entries);

    functions::add_math_functions(&mut entries);
    functions::add_geometric_functions(&mut entries);
    functions::add_relational_functions(&mut entries);
    functions::add_texture_functions(&mut entries);
    functions::add_synchronization_functions(&mut entries);
    functions::add_simd_functions(&mut entries);
    functions::add_atomic_functions(&mut entries);

    functions::add_attributes(&mut entries);
    functions::add_sampler_constants(&mut entries);
    functions::add_snippets(&mut entries);
    functions::add_raytracing_types(&mut entries);
    functions::add_misc_types(&mut entries);
    functions::add_builtin_constants(&mut entries);

    entries
}

pub fn all() -> &'static [BuiltinEntry] {
    ALL_BUILTINS.get_or_init(build_builtins)
}

pub fn lookup(name: &str) -> Option<&'static BuiltinEntry> {
    let map = BUILTIN_MAP.get_or_init(|| {
        let entries = all();
        let mut m = HashMap::with_capacity(entries.len());
        for (i, entry) in entries.iter().enumerate() {
            m.entry(entry.label.clone()).or_insert(i);
        }
        m
    });

    map.get(name).map(|&i| &all()[i])
}
