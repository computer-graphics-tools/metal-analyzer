use std::{hint::black_box, path::PathBuf, sync::Arc};

use criterion::{Criterion, criterion_group, criterion_main};
use futures::future::join_all;
use metal_analyzer::{DefinitionProvider, metal::compiler::compute_include_paths, syntax::SyntaxTree};
use tower_lsp::lsp_types::{Position, Url};

const FIXTURE_RELATIVE_PATH: &str = "matmul/gemv/shaders/gemv_like.metal";
const BURST_SIZE: usize = 24;
const CONCURRENT_SIZE: usize = 8;

#[derive(Clone)]
struct NavigationFixture {
    uri: Url,
    source: Arc<String>,
    include_paths: Arc<Vec<String>>,
    snapshot: Arc<SyntaxTree>,
    jump_positions: Arc<Vec<Position>>,
}

fn has_metal_compiler() -> bool {
    std::process::Command::new("xcrun").args(["--find", "metal"]).output().is_ok_and(|output| output.status.success())
}

fn fixture_cases_root() -> PathBuf {
    let fixtures_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    if let Ok(entries) = std::fs::read_dir(&fixtures_root) {
        for entry in entries.flatten() {
            let candidate = entry.path();
            if !candidate.is_dir() {
                continue;
            }
            if candidate.join("common/types.h").exists()
                && candidate.join("generated/matmul.h").exists()
                && candidate.join("matmul/gemv/shaders/gemv_like.metal").exists()
            {
                return candidate;
            }
        }
    }
    fixtures_root.join("cases")
}

fn fixture_path(relative_path: &str) -> PathBuf {
    fixture_cases_root().join(relative_path)
}

fn position_of(
    source: &str,
    needle: &str,
) -> Position {
    position_of_nth(source, needle, 0)
}

fn position_of_nth(
    source: &str,
    needle: &str,
    nth: usize,
) -> Position {
    assert!(!needle.is_empty(), "needle must not be empty");
    let mut from = 0usize;
    let mut current = 0usize;

    loop {
        let Some(idx) = source[from..].find(needle) else {
            panic!("needle not found: {needle}");
        };
        let absolute = from + idx;
        if current == nth {
            let before = &source[..absolute];
            let line = before.as_bytes().iter().filter(|&&b| b == b'\n').count() as u32;
            let col = before
                .rsplit_once('\n')
                .map(|(_, tail)| tail.chars().count() as u32)
                .unwrap_or_else(|| before.chars().count() as u32);
            return Position::new(line, col);
        }
        current += 1;
        from = absolute + needle.len();
    }
}

fn load_navigation_fixture() -> Option<NavigationFixture> {
    let path = fixture_path(FIXTURE_RELATIVE_PATH);
    let uri = Url::from_file_path(&path).ok()?;
    let source = std::fs::read_to_string(&path).ok()?;
    let root = fixture_cases_root();
    let include_paths = compute_include_paths(&path, Some(&[root]));

    let jump_positions = vec![
        position_of(&source, "local_template(sum.re)"),
        position_of(&source, "fixture::overloaded(base)"),
        position_of_nth(&source, "shape->rows", 0),
        position_of_nth(&source, "none.marker", 0),
    ];

    Some(NavigationFixture {
        uri,
        source: Arc::new(source.clone()),
        include_paths: Arc::new(include_paths),
        snapshot: Arc::new(SyntaxTree::parse(&source)),
        jump_positions: Arc::new(jump_positions),
    })
}

fn bench_goto_navigation(c: &mut Criterion) {
    if !has_metal_compiler() {
        c.bench_function("goto_navigation/skip_no_metal_compiler", |b| b.iter(|| ()));
        return;
    }

    let Some(fixture) = load_navigation_fixture() else {
        c.bench_function("goto_navigation/skip_missing_fixture", |b| b.iter(|| ()));
        return;
    };

    let provider = Arc::new(DefinitionProvider::new());
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");

    runtime.block_on({
        let provider = Arc::clone(&provider);
        let fixture = fixture.clone();
        async move {
            provider.index_document(&fixture.uri, fixture.source.as_str(), fixture.include_paths.as_slice()).await;
        }
    });

    c.bench_function("goto_navigation/warm_single_jump", |b| {
        let provider = Arc::clone(&provider);
        let fixture = fixture.clone();
        let runtime = &runtime;
        b.iter(|| {
            let result = runtime.block_on(async {
                provider
                    .provide(
                        &fixture.uri,
                        fixture.jump_positions[0],
                        fixture.source.as_str(),
                        fixture.include_paths.as_slice(),
                        fixture.snapshot.as_ref(),
                    )
                    .await
            });
            black_box(result);
        });
    });

    c.bench_function("goto_navigation/warm_burst_sequential", |b| {
        let provider = Arc::clone(&provider);
        let fixture = fixture.clone();
        let runtime = &runtime;
        b.iter(|| {
            runtime.block_on(async {
                for index in 0..BURST_SIZE {
                    let position = fixture.jump_positions[index % fixture.jump_positions.len()];
                    let result = provider
                        .provide(
                            &fixture.uri,
                            position,
                            fixture.source.as_str(),
                            fixture.include_paths.as_slice(),
                            fixture.snapshot.as_ref(),
                        )
                        .await;
                    black_box(result);
                }
            });
        });
    });

    c.bench_function("goto_navigation/warm_burst_concurrent", |b| {
        let provider = Arc::clone(&provider);
        let fixture = fixture.clone();
        let runtime = &runtime;
        b.iter(|| {
            let result = runtime.block_on(async {
                let tasks = (0..CONCURRENT_SIZE).map(|index| {
                    let provider = Arc::clone(&provider);
                    let fixture = fixture.clone();
                    async move {
                        let position = fixture.jump_positions[index % fixture.jump_positions.len()];
                        provider
                            .provide(
                                &fixture.uri,
                                position,
                                fixture.source.as_str(),
                                fixture.include_paths.as_slice(),
                                fixture.snapshot.as_ref(),
                            )
                            .await
                    }
                });
                join_all(tasks).await
            });
            black_box(result);
        });
    });
}

criterion_group!(benches, bench_goto_navigation);
criterion_main!(benches);
