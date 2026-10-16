use super::*;

#[cfg(windows)]
#[tokio::test]
async fn windows_compiler_defaults_to_metal4_and_allows_standard_override() {
    if find_metal_compiler().await.is_none() {
        return;
    }

    for (flags, expected) in [
        (Vec::<&str>::new(), "#define __METAL_VERSION__ 400"),
        (vec!["-std=macos-metal2.2"], "#define __METAL_VERSION__ 220"),
    ] {
        let output = metal_command()
            .args(flags)
            .args(["-dM", "-E", "-x", "metal", "-"])
            .stdin(std::process::Stdio::null())
            .output()
            .expect("run Windows Metal compiler");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stdout).lines().any(|line| line == expected));
    }
}

#[cfg(windows)]
fn temp_install_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "metal-analyzer-toolchain-{name}-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("clock drift").as_nanos()
    ));
    std::fs::create_dir_all(dir.join("bin")).expect("create bin dir");
    std::fs::write(dir.join("bin").join("metal.exe"), "").expect("write launcher");
    dir
}

#[cfg(windows)]
fn add_versioned_compiler(
    install_dir: &std::path::Path,
    version: &str,
) -> PathBuf {
    let bin = install_dir.join("metal").join(version).join("bin");
    std::fs::create_dir_all(&bin).expect("create versioned bin dir");
    let compiler = bin.join("metal.exe");
    std::fs::write(&compiler, "").expect("write versioned compiler");
    compiler
}

#[cfg(windows)]
#[test]
fn versioned_compiler_prefers_newest_numeric_version() {
    let install_dir = temp_install_dir("newest");
    add_versioned_compiler(&install_dir, "32023");
    let newest = add_versioned_compiler(&install_dir, "32100");
    add_versioned_compiler(&install_dir, "9999");
    std::fs::create_dir_all(install_dir.join("metal").join("not-a-version").join("bin")).expect("create stray dir");

    let resolved = windows::versioned_compiler(&install_dir.join("bin").join("metal.exe"));
    assert_eq!(resolved, Some(newest));

    std::fs::remove_dir_all(&install_dir).ok();
}

#[cfg(windows)]
#[test]
fn versioned_compiler_compares_dotted_versions_numerically() {
    let install_dir = temp_install_dir("dotted");
    add_versioned_compiler(&install_dir, "32023.9");
    let newest = add_versioned_compiler(&install_dir, "32023.10");

    let resolved = windows::versioned_compiler(&install_dir.join("bin").join("metal.exe"));
    assert_eq!(resolved, Some(newest));

    std::fs::remove_dir_all(&install_dir).ok();
}

#[cfg(windows)]
#[test]
fn versioned_compiler_is_none_outside_install_layout() {
    let install_dir = temp_install_dir("bare");

    assert_eq!(windows::versioned_compiler(&install_dir.join("bin").join("metal.exe")), None);

    std::fs::remove_dir_all(&install_dir).ok();
}

#[cfg(not(windows))]
#[test]
fn clang_format_falls_back_to_xcrun() {
    assert_eq!(clang_format_fallback(), Some(("xcrun".to_string(), vec!["clang-format".to_string()])));
}

#[cfg(not(windows))]
#[test]
fn metal_command_runs_through_xcrun() {
    let command = metal_command();
    assert_eq!(command.get_program(), "xcrun");
    assert_eq!(command.get_args().collect::<Vec<_>>(), ["metal"]);
}
