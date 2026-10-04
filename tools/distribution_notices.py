"""Collect source-backed notices, not legal clearance or a binary linkage claim."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import zipfile

NOTICE_NAME = re.compile(r"^(licen[cs]es?|copying|notice|copyright)([-_.].*)?$", re.I)
MAX_TEXT = 2 * 1024 * 1024
MAX_BUNDLE = 16 * 1024 * 1024
LEGACY_AUXILIARY_NAMES = {"THIRD-PARTY-NOTICES.txt", "dependency-inventory.json", "USAGE.md"}
PROJECT_LICENSE_NAMES = {"LICENSE-MIT", "LICENSE-APACHE"}
AUXILIARY_NAMES = LEGACY_AUXILIARY_NAMES | PROJECT_LICENSE_NAMES
LIMITS = [
    "Resolved normal/build dependency closure; not a linker-derived list of shipped code.",
    "Cargo feature unification and build dependencies can over-include components.",
    "Discovered license/notice files are preserved; embedded native code and toolchain notices require separate review.",
    "License expressions are upstream declarations, not selected license options or redistribution approval.",
    "The project license is MIT OR Apache-2.0; final redistribution acceptance remains an owner decision. Optional engines are separate packages.",
]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode(value: dict) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def bounded_text(path: Path, root: Path) -> bytes:
    # No license_file may import arbitrary host files through parent traversal or links.
    relative = path.relative_to(root)
    cursor = root
    for part in relative.parts:
        if part in ("", ".", ".."):
            raise ValueError("invalid notice source path")
        cursor = cursor / part
        if cursor.is_symlink():
            raise ValueError("notice source must not traverse symbolic links")
    info = path.stat()
    if not stat.S_ISREG(info.st_mode) or not 0 < info.st_size <= MAX_TEXT:
        raise ValueError("notice source is not a bounded regular file")
    with path.open("rb") as stream:
        data = stream.read(MAX_TEXT + 1)
    if len(data) > MAX_TEXT:
        raise ValueError("notice source grew beyond its bound")
    data.decode("utf-8")
    return data


def notice_sources(root: Path, explicit: str | None) -> list[tuple[str, bytes]]:
    selected: set[Path] = set()
    if explicit:
        path = Path(explicit)
        path = path if path.is_absolute() else root / path
        path.relative_to(root)  # Reject outside-package paths before reading.
        selected.add(path)
    visited = 0
    for directory, dirs, files in os.walk(root, followlinks=False):
        dirs[:] = sorted(d for d in dirs if d not in (".git", "target") and not (Path(directory) / d).is_symlink())
        visited += len(files) + len(dirs)
        if visited > 100000:
            raise ValueError("dependency notice traversal exceeds bound")
        for name in files:
            if NOTICE_NAME.fullmatch(name):
                selected.add(Path(directory) / name)
    return [(p.relative_to(root).as_posix(), bounded_text(p, root)) for p in sorted(selected)]


def dependency_closure(metadata: dict) -> tuple[list[dict], list[dict]]:
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    workspace = set(metadata["workspace_members"])
    roots = [key for key in workspace if packages[key]["name"] == "yu-cli"]
    if len(roots) != 1:
        raise ValueError("expected exactly one workspace yu-cli root")
    visited: set[str] = set()
    pending = roots[:]
    while pending:
        key = pending.pop()
        if key in visited:
            continue
        if key not in packages or key not in nodes or len(visited) >= 4096:
            raise ValueError("incomplete or oversized resolved dependency graph")
        visited.add(key)
        for edge in nodes[key]["deps"]:
            kinds = edge["dep_kinds"]
            if not kinds or any(k["kind"] not in (None, "build", "dev") for k in kinds):
                raise ValueError("unknown dependency kind")
            if any(k["kind"] != "dev" for k in kinds):
                pending.append(edge["pkg"])
    ordered = sorted((packages[k] for k in visited), key=lambda p: (p["name"], p["version"], p.get("source") or ""))
    return [p for p in ordered if p["id"] not in workspace], [p for p in ordered if p["id"] in workspace]


def cli_notices(metadata: dict, lock: dict, source: str, target: str) -> tuple[bytes, bytes]:
    packages, own = dependency_closure(metadata)
    checksums = {(p["name"], p["version"], p.get("source")): p.get("checksum") for p in lock["package"]}
    components, missing, sections = [], [], ["YuTool third-party source notices\n\n" + "\n".join(LIMITS) + "\n"]
    total = 0
    for package in packages:
        if not package.get("source"):
            raise ValueError("non-workspace path dependency requires an explicit provenance policy")
        root = Path(package["manifest_path"]).parent
        files = notice_sources(root, package.get("license_file"))
        component = {"name": package["name"], "version": package["version"], "source": package["source"],
                     "declared_license": package.get("license"), "package_checksum_from_lock": checksums.get((package["name"], package["version"], package["source"])),
                     "notice_files": []}
        label = f"{package['name']} {package['version']}"
        if not files:
            missing.append({"component": label, "reason": "no_notice_text_discovered"})
        if not package.get("license") and not package.get("license_file"):
            missing.append({"component": label, "reason": "no_license_declaration"})
        sections.append(f"\n{'=' * 72}\n{label}\nSource: {package['source']}\nDeclared license: {package.get('license') or 'not declared'}\n")
        for name, content in files:
            total += len(content)
            if total > MAX_BUNDLE:
                raise ValueError("notice bundle exceeds aggregate bound")
            component["notice_files"].append({"source_path": name, "sha256": sha256(content), "bytes": len(content)})
            sections.append(f"\n--- {name}; SHA-256 {sha256(content)} ---\n" + content.decode("utf-8") + "\n")
        components.append(component)
    inventory = {"schema_version": "1", "kind": "resolved_dependency_notice_inventory", "target": target, "source_commit": source,
                 "scope": "yu-cli resolved normal/build closure, target-filtered by Cargo; not exact binary linkage", "components": components,
                 "project_components": [{"name": p["name"], "version": p["version"], "declared_license": p.get("license")} for p in own],
                 "missing_notice_evidence": missing, "limitations": LIMITS,
                 "redistribution_review_accepted": False, "public_release_ready": False}
    return "".join(sections).encode("utf-8"), encode(inventory)


def engine_notice_inventory(archive: Path) -> dict:
    """Inspect existing .yu2 license payload without changing it or extracting any member."""
    with zipfile.ZipFile(archive) as z:
        names = z.namelist()
        if len(names) > 20000 or len(names) != len(set(names)):
            raise ValueError("ambiguous or oversized engine archive")

        observed_bytes = 0

        def read(name: str) -> bytes:
            nonlocal observed_bytes
            info = z.getinfo(name)
            if info.is_dir() or info.flag_bits & 1 or not 0 < info.file_size <= MAX_TEXT:
                raise ValueError("invalid engine notice/metadata member")
            with z.open(info) as stream:
                data = stream.read(MAX_TEXT + 1)
            if len(data) > MAX_TEXT:
                raise ValueError("oversized engine notice")
            observed_bytes += len(data)
            if observed_bytes > MAX_BUNDLE:
                raise ValueError("engine notice inventory exceeds aggregate bound")
            data.decode("utf-8")
            return data

        receipt = json.loads(read("engine/yu-engine.json"))
        node = read("licenses/node-LICENSE")
        components = [{"name": "node", "version": receipt["node_version"], "notice_files": [{"source_path": "licenses/node-LICENSE", "sha256": sha256(node), "bytes": len(node)}]}]
        for name in ("ag-psd", "base64-js", "pako"):
            prefix = f"engine/node_modules/{name}/"
            package = json.loads(read(prefix + "package.json"))
            if package.get("name") != name:
                raise ValueError("engine dependency identity mismatch")
            files = []
            for member in sorted(n for n in names if n.startswith(prefix) and NOTICE_NAME.fullmatch(PurePosixPath(n).name)):
                data = read(member)
                files.append({"source_path": member, "sha256": sha256(data), "bytes": len(data)})
            if not files:
                raise ValueError(f"engine dependency {name} has no notice payload")
            components.append({"name": name, "version": package["version"], "declared_license": package.get("license"), "notice_files": files})
        return {"schema_version": "1", "kind": "existing_engine_notice_inventory", "engine_id": receipt["engine_id"],
                "engine_version": receipt["engine_version"], "components": components,
                "redistribution_review_accepted": False, "public_release_ready": False,
                "limitations": ["Presence and hashes only, not legal clearance; unchanged existing engine payload."]}
