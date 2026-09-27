#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import stat
import tarfile
import tempfile
import urllib.request
import zipfile
from pathlib import Path


FIXED_ZIP_TIME = (1980, 1, 1, 0, 0, 0)
PACKAGE_MODULES = {
    "ag-psd": "31.0.2",
    "base64-js": "1.5.1",
    "pako": "2.1.0",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path):
    with path.open("r", encoding="utf-8") as stream:
        return json.load(stream)


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def target_key(target: dict) -> str:
    return f"{target['os']}-{target['arch']}"


def find_source(matrix: dict, os_name: str, arch: str) -> dict:
    for source in matrix["validated_targets"]:
        if source["target"] == {"os": os_name, "arch": arch}:
            return source
    raise SystemExit(f"target is not in validated package matrix: {os_name}/{arch}")


def download_verified(source: dict, destination: Path) -> None:
    request = urllib.request.Request(
        source["url"],
        headers={"User-Agent": "YuTool-ag-psd-package-builder/1"},
    )
    with urllib.request.urlopen(request, timeout=120) as response, destination.open("wb") as out:
        shutil.copyfileobj(response, out)

    observed = sha256_file(destination)
    if observed.lower() != source["sha256"].lower():
        raise SystemExit(
            f"Node archive SHA-256 mismatch for {source['archive']}: "
            f"expected {source['sha256']}, got {observed}"
        )


def extract_member(archive: Path, source: dict, member_name: str, destination: Path) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True)

    if source["archive_kind"] == "zip":
        with zipfile.ZipFile(archive) as zf:
            info = zf.getinfo(member_name)
            if info.is_dir():
                raise SystemExit(f"expected file member, got directory: {member_name}")
            with zf.open(info) as src, destination.open("wb") as out:
                shutil.copyfileobj(src, out)
        return

    if source["archive_kind"] == "tar_xz":
        with tarfile.open(archive, "r:xz") as tf:
            member = tf.getmember(member_name)
            if not member.isfile():
                raise SystemExit(f"expected regular file member: {member_name}")
            src = tf.extractfile(member)
            if src is None:
                raise SystemExit(f"cannot read archive member: {member_name}")
            with src, destination.open("wb") as out:
                shutil.copyfileobj(src, out)
        return

    raise SystemExit(f"unsupported source archive kind: {source['archive_kind']}")


def verify_node_modules(node_modules: Path) -> None:
    for package, expected_version in PACKAGE_MODULES.items():
        package_root = node_modules / package
        package_json = package_root / "package.json"
        if not package_json.is_file():
            raise SystemExit(f"missing npm package: {package}")
        actual = load_json(package_json).get("version")
        if actual != expected_version:
            raise SystemExit(
                f"npm package version mismatch for {package}: "
                f"expected {expected_version}, got {actual}"
            )


def copy_package_tree(source: Path, destination: Path) -> None:
    if source.is_symlink():
        raise SystemExit(f"npm package root must not be a symlink: {source}")
    destination.mkdir(parents=True, exist_ok=True)

    for item in sorted(source.rglob("*"), key=lambda path: path.as_posix()):
        relative = item.relative_to(source)
        target = destination / relative
        if item.is_symlink():
            raise SystemExit(f"npm package contains symlink: {item}")
        if item.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        elif item.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(item, target)


def zip_payload(payload: Path, output: Path, executable_path: str) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        for file in sorted(
            (path for path in payload.rglob("*") if path.is_file()),
            key=lambda path: path.relative_to(payload).as_posix(),
        ):
            relative = file.relative_to(payload).as_posix()
            info = zipfile.ZipInfo(relative, FIXED_ZIP_TIME)
            info.compress_type = zipfile.ZIP_DEFLATED
            mode = 0o755 if relative == executable_path else 0o644
            info.external_attr = (stat.S_IFREG | mode) << 16
            with file.open("rb") as stream:
                zf.writestr(info, stream.read(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)


def build(args) -> None:
    matrix = load_json(Path(args.sources))
    source = find_source(matrix, args.target_os, args.target_arch)
    node_modules = Path(args.node_modules).resolve()
    protocol_adapter = Path(args.protocol_adapter).resolve()
    output_dir = Path(args.output_dir).resolve()

    verify_node_modules(node_modules)
    if not protocol_adapter.is_file():
        raise SystemExit(f"protocol adapter not found: {protocol_adapter}")

    key = target_key(source["target"])
    package_name = (
        f"yu-engine-ag-psd-{matrix['engine_version'].replace('+', '-')}-{key}.zip"
    )
    base_url = args.package_base_url.rstrip("/")
    package_url = f"{base_url}/{package_name}"

    with tempfile.TemporaryDirectory(prefix="yu-ag-psd-package-") as temporary:
        temporary = Path(temporary)
        node_archive = temporary / source["archive"]
        download_verified(source, node_archive)

        payload = temporary / "payload"
        entrypoint = payload / source["package_entrypoint"]
        extract_member(node_archive, source, source["node_member"], entrypoint)
        if source["target"]["os"] != "windows":
            entrypoint.chmod(0o755)

        node_license = payload / "licenses" / "node-LICENSE"
        extract_member(node_archive, source, source["license_member"], node_license)

        engine_dir = payload / "engine"
        engine_dir.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(protocol_adapter, engine_dir / "ag_psd_protocol.cjs")

        packaged_modules = engine_dir / "node_modules"
        for package in PACKAGE_MODULES:
            copy_package_tree(node_modules / package, packaged_modules / package)

        receipt = {
            "schema_version": "1",
            "engine_id": matrix["engine_id"],
            "engine_version": matrix["engine_version"],
            "ag_psd_version": matrix["ag_psd_version"],
            "node_version": matrix["node_version"],
            "target": source["target"],
            "protocol_version": "1",
            "psd_contract_version": "1",
            "capabilities": matrix["package_capabilities"],
            "entrypoint": source["package_entrypoint"],
            "args": ["engine/ag_psd_protocol.cjs"],
            "source_node_archive": {
                "url": source["url"],
                "sha256": source["sha256"],
            },
        }
        write_json(engine_dir / "yu-engine.json", receipt)

        archive_path = output_dir / package_name
        zip_payload(payload, archive_path, source["package_entrypoint"])
        package_sha = sha256_file(archive_path)

    metadata = {
        "schema_version": "1",
        "engine_id": matrix["engine_id"],
        "engine_version": matrix["engine_version"],
        "ag_psd_version": matrix["ag_psd_version"],
        "node_version": matrix["node_version"],
        "target": source["target"],
        "archive": "zip",
        "file_name": package_name,
        "url": package_url,
        "sha256": package_sha,
        "bytes": archive_path.stat().st_size,
        "entrypoint": source["package_entrypoint"],
        "args": ["engine/ag_psd_protocol.cjs"],
        "capabilities": matrix["package_capabilities"],
    }
    metadata_path = output_dir / f"metadata-{key}.json"
    write_json(metadata_path, metadata)

    manifest = {
        "schema_version": "1",
        "id": matrix["engine_id"],
        "display_name": "YuTool Managed ag-psd Engine",
        "version": matrix["engine_version"],
        "capabilities": matrix["package_capabilities"],
        "packages": [
            {
                "target": source["target"],
                "url": package_url,
                "sha256": package_sha,
                "archive": "zip",
                "entrypoint": source["package_entrypoint"],
                "args": ["engine/ag_psd_protocol.cjs"],
            }
        ],
    }
    write_json(output_dir / f"manifest-{key}.json", manifest)

    print(json.dumps(metadata, ensure_ascii=False, sort_keys=True))


def assemble(args) -> None:
    metadata_files = [Path(path) for path in args.metadata]
    if not metadata_files:
        raise SystemExit("assemble requires at least one metadata file")

    records = [load_json(path) for path in metadata_files]
    first = records[0]
    for record in records[1:]:
        for field in ["engine_id", "engine_version", "ag_psd_version", "node_version"]:
            if record[field] != first[field]:
                raise SystemExit(f"metadata disagreement for {field}")

    packages = []
    for record in sorted(
        records,
        key=lambda record: (record["target"]["os"], record["target"]["arch"]),
    ):
        packages.append(
            {
                "target": record["target"],
                "url": record["url"],
                "sha256": record["sha256"],
                "archive": record["archive"],
                "entrypoint": record["entrypoint"],
                "args": record["args"],
            }
        )

    manifest = {
        "schema_version": "1",
        "id": first["engine_id"],
        "display_name": "YuTool Managed ag-psd Engine",
        "version": first["engine_version"],
        "capabilities": first["capabilities"],
        "packages": packages,
    }
    write_json(Path(args.output), manifest)
    print(json.dumps(manifest, ensure_ascii=False, sort_keys=True))


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)

    build_parser = sub.add_parser("build")
    build_parser.add_argument("--sources", required=True)
    build_parser.add_argument("--target-os", required=True)
    build_parser.add_argument("--target-arch", required=True)
    build_parser.add_argument("--node-modules", required=True)
    build_parser.add_argument("--protocol-adapter", required=True)
    build_parser.add_argument("--output-dir", required=True)
    build_parser.add_argument("--package-base-url", required=True)
    build_parser.set_defaults(func=build)

    assemble_parser = sub.add_parser("assemble")
    assemble_parser.add_argument("--metadata", nargs="+", required=True)
    assemble_parser.add_argument("--output", required=True)
    assemble_parser.set_defaults(func=assemble)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
