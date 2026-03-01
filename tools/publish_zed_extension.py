# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///

import base64
import json
import os
import re
from urllib.error import HTTPError
from urllib.request import Request, urlopen


FORK     = "computer-graphics-tools/extensions"
UPSTREAM = "zed-industries/extensions"
EXT_PATH = "extensions/metal"


def api(token: str, method: str, path: str, body: dict | None = None) -> dict:
    url  = f"https://api.github.com/{path}"
    data = json.dumps(body).encode() if body else None
    req  = Request(url, data=data, method=method, headers={
        "Authorization":        f"token {token}",
        "Accept":               "application/vnd.github+json",
        "Content-Type":         "application/json",
        "X-GitHub-Api-Version": "2022-11-28",
    })
    try:
        with urlopen(req) as r:
            return json.load(r)
    except HTTPError as e:
        raise RuntimeError(f"HTTP {e.code} {method} {path}: {e.read().decode()}") from e


def sync_fork(token: str) -> None:
    try:
        api(token, "POST", f"repos/{FORK}/merge-upstream", {"branch": "main"})
    except RuntimeError as e:
        if "409" not in str(e):
            raise


def updated_extensions_toml(content: str, version: str) -> str:
    new_content = re.sub(
        r"(\[metal\](?:[^\[]*?)version = \")[^\"]+(\")",
        lambda m: m.group(1) + version + m.group(2),
        content,
    )
    if new_content == content:
        raise RuntimeError("no replacements occurred in extensions.toml")
    return new_content


def main() -> None:
    token      = os.environ["COMMITTER_TOKEN"]
    tag        = os.environ["RELEASE_TAG"]
    version    = tag.lstrip("v")
    commit_sha = os.environ["RELEASE_SHA"]
    run_id     = os.environ.get("GITHUB_RUN_ID", "local")
    branch     = f"update-metal-{run_id}"

    sync_fork(token)

    main_sha = api(token, "GET", f"repos/{FORK}/git/ref/heads/main")["object"]["sha"]

    api(token, "POST", f"repos/{FORK}/git/refs", {
        "ref": f"refs/heads/{branch}",
        "sha": main_sha,
    })

    file_info   = api(token, "GET", f"repos/{FORK}/contents/extensions.toml?ref={branch}")
    old_content = base64.b64decode(file_info["content"]).decode("utf-8")
    new_content = updated_extensions_toml(old_content, version)

    tree_sha = api(token, "POST", f"repos/{FORK}/git/trees", {
        "base_tree": main_sha,
        "tree": [
            {"path": EXT_PATH,          "mode": "160000", "type": "commit", "sha":     commit_sha},
            {"path": "extensions.toml", "mode": "100644", "type": "blob",   "content": new_content},
        ],
    })["sha"]

    commit_sha_new = api(token, "POST", f"repos/{FORK}/git/commits", {
        "message": (
            f"Update metal to v{version}\n\n"
            f"Release notes:\n\n"
            f"https://github.com/computer-graphics-tools/metal-analyzer/releases/tag/v{version}"
        ),
        "tree":    tree_sha,
        "parents": [main_sha],
    })["sha"]

    api(token, "PATCH", f"repos/{FORK}/git/refs/heads/{branch}", {"sha": commit_sha_new})

    pr = api(token, "POST", f"repos/{UPSTREAM}/pulls", {
        "title": f"Update metal to v{version}",
        "body":  f"Release notes:\n\nhttps://github.com/computer-graphics-tools/metal-analyzer/releases/tag/v{version}",
        "base":  "main",
        "head":  f"computer-graphics-tools:{branch}",
    })
    print(f"PR created: {pr['html_url']}")


if __name__ == "__main__":
    main()
