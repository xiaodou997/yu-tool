#!/usr/bin/env python3
"""Stage immutable, invite-only YuTool CLI development preview bundles.

Repackages exact frozen-main GitHub Actions ZIPs as opaque nested archives;
never changes binaries, licenses, candidate metadata, or engine archives.
Requires six original ZIPs and the previously frozen exact-main
isolated-acceptance receipts. No tag/Release/upload/signing/notarization.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
import zipfile

import audit_v01_postmerge_artifacts as audit


IDENTITY = "yu-v0.1.0-dev.5526dc4"
PLATFORMS = {
    "macos": "macos-arm64",
    "windows": "windows-x86_64",
    "linux": "linux-x86_64",
}
ROOT_README = """# YuTool v0.1.0-dev — 内部测试发行

本目录包含三个平台的 **CLI 开发测试 ZIP**，并非公开稳定版本或 GUI 安装程序。
冻结代码为 `main@5526dc4465c4e79ad7f2757d06b3b1693642b8af`。

1. 按设备选择 macos-arm64、windows-x86_64 或 linux-x86_64 对应的 ZIP。
2. 分发前通过 `SHA256SUMS` 核验 ZIP；仅交付知晓风险的受邀测试人员。
3. 解压平台 ZIP，先看其中的 `README-DEV.zh-CN.md`。
4. `candidate/` 为完整的原始 CLI 候选 ZIP，`engine/` 为可选 PSD 引擎原始 ZIP 与离线清单。
5. 所有写入类图片操作只针对测试副本；提供脱敏故障日志，不传个人 PSD。

**尚未公证，不支持公开分发或自动更新；Windows 为未签名预览。**
不全局关闭 macOS Gatekeeper 或 Windows SmartScreen。
Windows Issue #27：外部文件占用可能暂时阻碍引擎卸载，原安装版本保持不变。
不包含 PSD 写入和图层重命名功能。引擎清单中的 example.invalid 是占位地址，
只允许使用 engine/manifest.json 与本地 ZIP 显式离线安装。
"""
README = {
    "macos": """# YuTool v0.1.0-dev — macOS Apple Silicon

**邀请制内部开发测试 / Private developer preview only.**
Source: main@5526dc4465c4e79ad7f2757d06b3b1693642b8af
This CLI is NOT a signed/notarized end-user macOS application.
It is a command-line tool, not a desktop GUI or installer.

## 试用

1. 先解压这个平台内测包，再解压 candidate/ 目录中的 YuTool CLI ZIP。
2. 在解压后的 CLI 目录运行 `./bin/yu --version` 和 `./bin/yu doctor --json`。
3. 执行 `./bin/yu image info picture.png --json`，再在您自行准备的**副本**上测试图像操作。
4. 可选的 PSD 只读功能，需要将 engine/ 中的独立 ZIP 显式离线安装：
   `./bin/yu engine install --manifest /absolute/path/to/engine/manifest.json --archive /absolute/path/to/engine/ag-psd-engine.zip --json`
   然后 `./bin/yu engine activate ag-psd 31.0.2+node22.23.3.yu2 --json`。
5. 使用副本测试 `./bin/yu psd inspect example.psd --json`。

本测试包未经 Apple 公证。macOS Gatekeeper 可能阻止首次打开，请仅在您信任来源
且核对 SHA256SUMS 后，根据 macOS「系统设置 > 隐私与安全性」提供的单个应用允许流程操作。
**不要全局关闭 Gatekeeper，不要使用 sudo，不要覆盖重要文件。**
如需删除测试数据，请先通过 CLI 自行卸载测试引擎，并确认没有其他版本共用数据目录。

本预览不支持 PSD 写入、更改图层名称、正式自动更新或公开分发。
候选包内部 URL 是占位地址；使用 `--archive` 本地安装，不使用占位 URL。
候选内 `public_release_ready=false` 是刻意保留的安全边界。
""",
    "windows": """# YuTool v0.1.0-dev — Windows 11 x64

**邀请制内部开发测试 / Private developer preview only.**
Source: main@5526dc4465c4e79ad7f2757d06b3b1693642b8af
Unsigned developer ZIP, NOT an installer, Authenticode-signed release,
or an ordinary consumer application. SmartScreen may warn or block it.

## 使用 PowerShell

1. 解压这个平台测试包，再解压 candidate/ 中的 CLI ZIP。
2. 运行 `.\\bin\\yu.exe --version`、`.\\bin\\yu.exe doctor --json`。
3. 在自己准备的图片/PSD **副本** 上测试只读操作。
4. 可选的 PSD 引擎需要用 engine/ 中的原始 ZIP 离线安装：
   `.\\bin\\yu.exe engine install --manifest C:\\path\\to\\engine\\manifest.json --archive C:\\path\\to\\engine\\ag-psd-engine.zip --json`
   `.\\bin\\yu.exe engine activate ag-psd 31.0.2+node22.23.3.yu2 --json`
5. 试运行 `.\\bin\\yu.exe psd inspect example.psd --json`。

此开发包可能触发 SmartScreen/信誉提示；只允许可信来源的已核验包，
不要全局停用 Defender/SmartScreen 或使用管理员权限强行解除拦截。

已知风险 Issue #27：第三方安全扫描器或文件占用者可能暂时阻碍 Managed
engine 卸载。YuTool 会有界重试并保留原安装版本；等待占用释放再重试，
不要强制终止其他进程。本版本不开放 PSD 修改、自动更新或公开分发。
""",
    "linux": """# YuTool v0.1.0-dev — Linux x86_64

**邀请制内部开发测试 / Private developer preview only.**
Source: main@5526dc4465c4e79ad7f2757d06b3b1693642b8af
CLI ZIP only, not a system installer or official public release.
Ubuntu 24.04 amd64 has one separate clean-container floor acceptance;
other distributions are not claimed compatible.

## 试用

1. 解压这个平台包，再解压 candidate/ 中的 CLI ZIP。
2. 运行 `./bin/yu --version`、`./bin/yu doctor --json`。
3. 如需试用 PSD 只读：使用 engine/ 原始 ZIP，显式安装：
   `./bin/yu engine install --manifest /absolute/path/to/engine/manifest.json --archive /absolute/path/to/engine/ag-psd-engine.zip --json`
   `./bin/yu engine activate ag-psd 31.0.2+node22.23.3.yu2 --json`
   `./bin/yu psd inspect example.psd --json`。

仅对副本执行写入类图像操作，不使用 sudo 安装内测包。
不支持 PSD 编辑、自动更新或公开分发。
""",
}


def ensure_regular(source: Path) -> bytes:
    if not source.is_file() or source.is_symlink():
        raise ValueError(f"untrusted source: {source}")
    if not 0 < source.stat().st_size <= 250 * 1024 * 1024:
        raise ValueError(f"unbounded private preview input: {source.name}")
    return source.read_bytes()


def owned_zip_file(f: Path, expected_sha: str) -> bytes:
    raw = ensure_regular(f)
    if hashlib.sha256(raw).hexdigest() != expected_sha:
        raise ValueError(f"frozen binary/package hash changed: {f.name}")
    return raw


def build(artifacts: Path, dest: Path, repo: Path) -> dict:
    # The existing audit is the publication input gate, not just a summary.
    evidence = audit.audit(artifacts, repo)
    if evidence["technical_artifacts_audit"] != "PASS":
        raise ValueError("exact-main validation is not PASS")
    if dest.exists() or dest.is_symlink():
        raise FileExistsError(f"internal preview output already exists: {dest}")
    if not dest.parent.is_dir() or dest.parent.is_symlink():
        raise ValueError("private output parent must be an existing real directory")
    dest.mkdir(mode=0o700)
    manifest = {
        "schema_version": "1",
        "kind": "restricted_internal_developer_preview",
        "release": IDENTITY,
        "source_commit": audit.MAIN,
        "source_tree": audit.TREE,
        "input_managed_package_run": 37342387756,
        "channel": "invite_only_manual_delivery",
        "development_test_only": True,
        "notary_submitted": False,
        "public_release_ready": False,
        "public_release_authorized": False,
        "release_tag_created": False,
        "automatic_update": False,
        "psd_mutation_allowed": False,
        "underlying_cli_and_engine_archives_modified": False,
        "platform_bundles": {},
    }
    for platform, name in PLATFORMS.items():
        cli_dir = artifacts / platform
        engine_dir = artifacts / f"managed-{platform}"
        archive = next(cli_dir.glob("*.zip"))
        managed = next(engine_dir.glob("*.zip"))
        cli_data=owned_zip_file(archive,
            evidence["cli_candidate"][platform]["archive_sha256"])
        managed_data=owned_zip_file(managed,
            evidence["managed_engine"][platform]["archive_sha256"])
        cjson=ensure_regular(cli_dir/"candidate.json")
        mjson=ensure_regular(next(engine_dir.glob("manifest-*.json")))
        meta=ensure_regular(next(engine_dir.glob("metadata-*.json")))
        with zipfile.ZipFile(archive) as z:
            mit=z.read("LICENSE-MIT")
            apache=z.read("LICENSE-APACHE")
            third_party=z.read("THIRD-PARTY-NOTICES.txt")
        items = {
            "README-DEV.zh-CN.md": README[platform].encode(),
            "LICENSE-MIT": mit,
            "LICENSE-APACHE":apache,
            "THIRD-PARTY-NOTICES.txt":third_party,
            f"candidate/{archive.name}":cli_data,
            "candidate/candidate.json":cjson,
            f"engine/{managed.name}":managed_data,
            "engine/manifest.json":mjson,
            "engine/metadata.json":meta,
        }
        # A nested archive is intentionally unmodified. The parent
        # bundle carries checksums of every exact inner payload.
        bundle_json={
            "version":IDENTITY,"source_commit":audit.MAIN,"platform":platform,
            "restricted_internal_test":True,
            "public_release_ready":False,
            "no_notarization":True,
            "psd_write_enabled":False,
            "payload_sha256":{k:hashlib.sha256(b).hexdigest()
                              for k,b in sorted(items.items())},
        }
        items["BUNDLE.json"]=json.dumps(bundle_json,sort_keys=True,indent=2).encode()+b"\n"
        checksum = "".join(f"{hashlib.sha256(b).hexdigest()}  {k}\n"
                           for k,b in sorted(items.items()))
        items["SHA256SUMS"]=checksum.encode()
        outfile=dest/f"{IDENTITY}-{name}.zip"
        if outfile.exists(): raise FileExistsError(outfile)
        with zipfile.ZipFile(outfile,"x",compression=zipfile.ZIP_STORED) as z:
            for key,data in sorted(items.items()):
                z.writestr(key,data)
        data=outfile.read_bytes()
        manifest["platform_bundles"][platform]={
            "file":outfile.name,"bytes":len(data),
            "sha256":hashlib.sha256(data).hexdigest(),
            "nested_cli_sha256":evidence["cli_candidate"][platform]["archive_sha256"],
            "nested_engine_sha256":evidence["managed_engine"][platform]["archive_sha256"],
            "source_exact_main":True,"internal_test_only":True,
        }
    checksums = "".join(f"{row['sha256']}  {row['file']}\n"
        for platform,row in sorted(manifest["platform_bundles"].items()))
    (dest/"SHA256SUMS").write_text(checksums)
    (dest/"INTERNAL-DEV-MANIFEST.json").write_text(
        json.dumps(manifest,sort_keys=True,indent=2,ensure_ascii=False)+"\n")
    (dest/"README-START-HERE.zh-CN.md").write_text(ROOT_README)
    return manifest


def verify(dest: Path, repo: Path) -> dict:
    manifest=json.loads((dest/"INTERNAL-DEV-MANIFEST.json").read_text())
    if manifest["source_commit"]!=audit.MAIN or manifest["source_tree"]!=audit.TREE:
        raise ValueError("internal preview source is not frozen main")
    if not manifest["development_test_only"] or manifest["public_release_authorized"]:
        raise ValueError("internal preview falsely upgraded to public release")
    if manifest["psd_mutation_allowed"] or manifest["underlying_cli_and_engine_archives_modified"]:
        raise ValueError("unsupported dev preview capabilities")
    platforms=set(PLATFORMS)
    if set(manifest["platform_bundles"])!=platforms:
        raise ValueError("three platform bundles required")
    checksum_lines=[]
    for platform,row in sorted(manifest["platform_bundles"].items()):
        f=dest/row["file"]
        content=ensure_regular(f)
        if len(content)!=row["bytes"] or hashlib.sha256(content).hexdigest()!=row["sha256"]:
            raise ValueError("modified platform internal ZIP")
        checksum_lines.append(f"{row['sha256']}  {row['file']}\n")
        with zipfile.ZipFile(f) as z:
            names=z.namelist()
            if len(names)!=len(set(names)) or len(names)!=11:
                raise ValueError("unexpected private bundle layout")
            data=json.loads(z.read("BUNDLE.json"))
            if data["platform"]!=platform or data["source_commit"]!=audit.MAIN or data["public_release_ready"]:
                raise ValueError("untrusted internal bundle manifest")
            actual=set(names)
            if actual != set(data["payload_sha256"])|{"BUNDLE.json","SHA256SUMS"}:
                raise ValueError("private bundle has unapproved paths")
            checksum_text=z.read("SHA256SUMS").decode()
            # Per-bundle SHA256SUMS covers the nine payloads PLUS the
            # BUNDLE.json manifest itself, but cannot include itself.
            expected_hashes={**data["payload_sha256"],
                      "BUNDLE.json":hashlib.sha256(z.read("BUNDLE.json")).hexdigest()}
            expected_sum="".join(f"{v}  {k}\n" for k,v in sorted(expected_hashes.items()))
            if checksum_text!=expected_sum:
                raise ValueError("per-file checksum list incomplete or modified")
            for name,hash_expected in data["payload_sha256"].items():
                if hashlib.sha256(z.read(name)).hexdigest()!=hash_expected:
                    raise ValueError("tampered nested bundle payload "+name)
            cli=[n for n in names if n.startswith("candidate/") and n.endswith(".zip")]
            engine=[n for n in names if n.startswith("engine/") and n.endswith(".zip")]
            if len(cli)!=1 or len(engine)!=1:
                raise ValueError("missing wrapped original archives")
            if hashlib.sha256(z.read(cli[0])).hexdigest()!=row["nested_cli_sha256"]:
                raise ValueError("modified underlying CLI ZIP")
            if hashlib.sha256(z.read(engine[0])).hexdigest()!=row["nested_engine_sha256"]:
                raise ValueError("modified underlying managed engine ZIP")
            if not z.read("README-DEV.zh-CN.md").startswith(b"# YuTool v0.1.0-dev"):
                raise ValueError("missing explicit dev-only instructions")
    if (dest/"SHA256SUMS").read_text()!="".join(checksum_lines):
        raise ValueError("top level SHA256SUMS mismatch")
    if (dest/"README-START-HERE.zh-CN.md").read_text()!=ROOT_README:
        raise ValueError("private preview README is missing or altered")
    return {"status":"PASS","bundles":len(platforms),"source_commit":audit.MAIN,
            "public_release_ready":False,"public_release_authorized":False}


def main() -> None:
    parser=argparse.ArgumentParser()
    parser.add_argument("--artifacts-dir",type=Path)
    parser.add_argument("--destination",type=Path,required=True)
    parser.add_argument("--repository",type=Path,default=Path.cwd())
    parser.add_argument("--verify-only",action="store_true")
    args=parser.parse_args()
    if args.verify_only:
        result=verify(args.destination.resolve(),args.repository.resolve())
    else:
        if args.artifacts_dir is None:
            parser.error("--artifacts-dir is required for staging")
        manifest=build(args.artifacts_dir.resolve(),args.destination.resolve(),
                       args.repository.resolve())
        result=verify(args.destination.resolve(),args.repository.resolve())
        result["bundle_files"]=[x["file"] for x in manifest["platform_bundles"].values()]
    print(json.dumps(result,sort_keys=True,ensure_ascii=False))


if __name__=="__main__":
    try: main()
    except (OSError,ValueError,KeyError,zipfile.BadZipFile) as error:
        sys.exit(f"PRIVATE_PREVIEW_REJECTED: {error}")
