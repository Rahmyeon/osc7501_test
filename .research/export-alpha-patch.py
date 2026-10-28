"""Export the complete feature patch, including new files, without staging it."""
from pathlib import Path
import hashlib
import json
import subprocess

root = Path(__file__).resolve().parent.parent
repo = root / ".research" / "wezterm"
reference = root / "wezterm"
alpha = root / "alpha"


def git(*args, cwd=repo):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, check=True).stdout


patch = bytearray(git("diff", "--binary", "HEAD"))
new_files = git("ls-files", "--others", "--exclude-standard", "-z").decode().split("\0")
for filename in filter(None, new_files):
    result = subprocess.run(
        ["git", "diff", "--no-index", "--binary", "--", "/dev/null", filename],
        cwd=repo, capture_output=True,
    )
    if result.returncode not in (0, 1):
        raise RuntimeError(result.stderr.decode(errors="replace"))
    patch.extend(result.stdout)
patch_path = alpha / "osc7501-alpha.patch"
patch_path.write_bytes(patch)

# A check against the untouched reference verifies that the export includes
# new modules and is applicable to the recorded clean baseline. No files move.
git("apply", "--check", str(patch_path), cwd=reference)
changed = git("diff", "--name-only", "HEAD", "-z").decode().split("\0")
files = sorted(set(filter(None, changed + new_files)))
manifest = {
    "source_revision": git("rev-parse", "HEAD").decode().strip(),
    "patch_sha256": hashlib.sha256(patch).hexdigest(),
    "applies_to_clean_reference": True,
    "files": [
        {"path": filename, "sha256": hashlib.sha256((repo / filename).read_bytes()).hexdigest()}
        for filename in files
    ],
}
(alpha / "source-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
print(f"Exported {len(files)} changed/new source files; patch applies to the clean baseline.")
