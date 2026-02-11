mod common;

use common::fixture_path;

#[derive(Debug)]
struct Scenario {
    id: &'static str,
    fixture_examples: &'static [&'static str],
    covered_by_tests: &'static [&'static str],
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        id: "include_graph_complexity",
        fixture_examples: &[
            "matmul/gemv/shaders/gemv_like.metal",
            "matmul/gemv/shaders/deep_include_error.metal",
        ],
        covered_by_tests: &[
            "goto_def_include_resolves_generated_header",
            "diagnostics_report_deep_header_error_on_header_file",
        ],
    },
    Scenario {
        id: "basename_collisions",
        fixture_examples: &[
            "common/utils.h",
            "matmul/common/steel/utils.h",
            "matmul/common/loader.h",
            "matmul/common/steel/gemm/loader.h",
        ],
        covered_by_tests: &[
            "goto_def_prefers_qualified_fixture_transform",
            "goto_def_prefers_qualified_steel_transform",
            "goto_def_prefers_qualified_fixture_loader",
            "goto_def_prefers_qualified_steel_loader",
        ],
    },
    Scenario {
        id: "include_ambiguity_resolution",
        fixture_examples: &[
            "matmul/gemv/shaders/ambiguous_loader_include.metal",
            "matmul/gemv/shaders/loader.h",
        ],
        covered_by_tests: &["goto_def_unqualified_loader_resolves_local_header"],
    },
    Scenario {
        id: "macro_redefinition_and_notes",
        fixture_examples: &["common/defines.h", "matmul/gemv/shaders/gemv_like.metal"],
        covered_by_tests: &[
            "diagnostics_include_macro_redefinition_note_pair",
            "edit_workflow_macro_warning_disappears_after_fix",
        ],
    },
    Scenario {
        id: "template_overload_namespace_mix",
        fixture_examples: &[
            "matmul/common/template_math.h",
            "matmul/common/transforms.h",
            "matmul/common/steel/gemm/transforms.h",
        ],
        covered_by_tests: &[
            "goto_def_overloaded_symbol_resolves_function_definition",
            "goto_def_prefers_qualified_fixture_transform",
            "goto_def_prefers_qualified_steel_transform",
        ],
    },
    Scenario {
        id: "conditional_compilation_generated_headers",
        fixture_examples: &["generated/matmul.h", "common/defines.h"],
        covered_by_tests: &["goto_def_include_resolves_generated_header"],
    },
    Scenario {
        id: "kernel_attributes_and_address_spaces",
        fixture_examples: &["matmul/gemv/shaders/gemv_like.metal"],
        covered_by_tests: &[
            "symbol_extraction_keeps_kernel_and_template_symbols",
            "semantic_tokens_cover_realistic_fixture_case",
        ],
    },
    Scenario {
        id: "header_owner_context",
        fixture_examples: &[
            "matmul/common/problematic_owner_only.h",
            "matmul/gemv/shaders/owner_context.metal",
        ],
        covered_by_tests: &["header_open_and_change_use_owner_context_diagnostics"],
    },
    Scenario {
        id: "cross_file_refs_and_rename",
        fixture_examples: &[
            "common/math_ops.h",
            "matmul/gemv/shaders/ref_user_a.metal",
            "matmul/gemv/shaders/ref_user_b.metal",
        ],
        covered_by_tests: &[
            "references_include_cross_file_uses_for_shared_symbol",
            "prepare_rename_allows_project_symbol",
        ],
    },
];

#[test]
fn every_scenario_has_mapping_to_tests() {
    for scenario in SCENARIOS {
        assert!(
            !scenario.covered_by_tests.is_empty(),
            "scenario '{}' must map to at least one test",
            scenario.id
        );
        assert!(
            !scenario.fixture_examples.is_empty(),
            "scenario '{}' must include fixture examples",
            scenario.id
        );
    }
}

#[test]
fn scenario_fixture_examples_exist() {
    for scenario in SCENARIOS {
        for rel in scenario.fixture_examples {
            let path = fixture_path(rel);
            assert!(
                path.exists(),
                "scenario '{}' references missing fixture: {}",
                scenario.id,
                path.display()
            );
        }
    }
}
