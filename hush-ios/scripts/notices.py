#!/usr/bin/env python3
"""Generate bundled notices from the locked iOS dependency graph."""
import json
from pathlib import Path
import subprocess
from urllib.request import urlopen

app = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--format-version", "1",
    "--filter-platform", "aarch64-apple-ios", "--manifest-path",
    str(app / "Bridge/Cargo.toml"),
]))
nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
seen = set()
remaining = [metadata["resolve"]["root"]]
while remaining:
    package_id = remaining.pop()
    if package_id in seen:
        continue
    seen.add(package_id)
    remaining.extend(dep["pkg"] for dep in nodes[package_id]["deps"]
                     if any(kind["kind"] is None for kind in dep["dep_kinds"]))

# These published crates omit the license file. The URLs use their recorded
# source commits, not a moving branch. Refresh them when the lockfile changes.
missing = {
    ("defmt-parser", "1.0.0"): "https://raw.githubusercontent.com/knurling-rs/defmt/4a8cdb44891ed57b8ff5a023b6bec7137c48708f/LICENSE-MIT",
    ("tsify-macros", "0.5.8"): "https://raw.githubusercontent.com/madonoharu/tsify/c16d520b784080809c20d004149cbbeb1d117dfc/LICENSE_MIT",
    ("validator", "0.20.0"): "https://raw.githubusercontent.com/Keats/validator/32e49fbd17eb4ca3d51e4fb5ac0180ea925fe605/LICENSE",
}
groups = {}
count = 0
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["id"] not in seen or not package["source"]:
        continue
    root = Path(package["manifest_path"]).parent
    texts = []
    for entry in sorted(root.iterdir()):
        if not entry.name.lower().startswith(("license", "copying", "notice", "copyright")):
            continue
        files = [entry] if entry.is_file() else sorted(p for p in entry.rglob("*") if p.is_file())
        texts.extend(p.read_text().strip() for p in files)
    source = package["repository"] or "https://crates.io/crates/" + package["name"]
    if not texts:
        url = missing.get((package["name"], package["version"]))
        if not url:
            raise SystemExit("Missing notice for " + package["name"])
        with urlopen(url, timeout=30) as response:
            texts = [response.read().decode().strip()]
        source = url
    notice = "\n\n".join(texts)
    groups.setdefault(notice, []).append(f"{package['name']} {package['version']}\n{source}")
    count += 1

output = ["Hush third-party notices", "Generated from Bridge/Cargo.lock for the iOS runtime dependency graph.",
          "The Hush source license does not replace the licenses of these dependencies."]
for notice, packages in groups.items():
    output.extend(["\n" + "=" * 72, "\n\n".join(packages), "", notice])
(app / "Resources/ThirdPartyNotices.txt").write_text("\n".join(output) + "\n")
print(f"Wrote notices for {count} packages in {len(groups)} groups.")
