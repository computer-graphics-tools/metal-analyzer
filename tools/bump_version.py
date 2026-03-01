# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///

import re
import sys
from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def bump_cargo_toml(root: Path, version: str) -> None:
    path = root / "Cargo.toml"
    path.write_text(
        re.sub(r'^version = "[^"]*"', f'version = "{version}"', path.read_text(), count=1, flags=re.MULTILINE)
    )


def bump_extension_toml(root: Path, version: str) -> None:
    path = root / "editors/zed/extension.toml"
    path.write_text(
        re.sub(r'^version = "[^"]*"', f'version = "{version}"', path.read_text(), count=1, flags=re.MULTILINE)
    )


def bump_package_json(root: Path, version: str) -> None:
    path = root / "editors/code/package.json"
    path.write_text(
        re.sub(r'"version": "[^"]*"', f'"version": "{version}"', path.read_text(), count=1)
    )


def bump_gradle_properties(root: Path, version: str) -> None:
    path = root / "editors/intellij/gradle.properties"
    path.write_text(
        re.sub(r'^pluginVersion = .*', f'pluginVersion = {version}', path.read_text(), flags=re.MULTILINE)
    )


def main() -> None:
    if len(sys.argv) != 2:
        print(f"Usage: uv run {sys.argv[0]} <new-version>", file=sys.stderr)
        print(f"Example: uv run {sys.argv[0]} 0.1.12", file=sys.stderr)
        sys.exit(1)

    version = sys.argv[1]
    root = repo_root()

    bump_cargo_toml(root, version)
    bump_extension_toml(root, version)
    bump_package_json(root, version)
    bump_gradle_properties(root, version)

    print(f"Bumped all versions to {version}")


if __name__ == "__main__":
    main()
