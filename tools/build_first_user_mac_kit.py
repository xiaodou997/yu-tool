#!/usr/bin/env python3
"""Create an offline first-user Mac acceptance KIT from the exact v0.1-dev.

The script checks the frozen preview ZIP, script and sample fixture
identities. It never publishes online, changes Git's frozen main,
removes quarantine metadata, signs, notarizes or edits a user's PSD.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
import zipfile

from accept_cli_candidate import tiny_png
from build_v01_dev_private_preview import verify as verify_preview

SOURCE="5526dc4465c4e79ad7f2757d06b3b1693642b8af"
LABEL="yu-v0.1.0-dev.5526dc4-mac-first-user"
MAC_ZIP="yu-v0.1.0-dev.5526dc4-macos-arm64.zip"
MAC_SHA="5897360ddbc770e45232b2bbbf569cb585a88d38547105a6ae872b6e72c8e44a"
PSD_SHA="406ddf7cbf5a1065b992b9c0d2a59be6290a371b90f23b9a04c0e6bbc94beb0c"
PNG_SHA="7f2538a6e81af23e034f8ca2efc2cb5f1d7d709f502fa30bdccfebe774e4ecab"

QUICKSTART="""# YuTool v0.1.0-dev — MacBook Air 第一轮真实用户验收

**仅供你本人或受邀测试人员使用。不是正式版，也不是图形界面 App。**
源码固定：`main@5526dc4465c4e79ad7f2757d06b3b1693642b8af`。

## 先做这五步

1. 将**整个** `yu-v0.1.0-dev.5526dc4-first-user-macos-arm64.zip` 传到 MacBook Air。
   推荐 AirDrop 或你信任的私有文件传输方式；不要公开上传。
2. 在 MacBook Air 上解压。进入解压后的 `YuTool-First-User-Mac` 目录。
3. 打开「终端」，先执行：
   `bash First-User-Test.command --verify-only`
   这个命令只验证原始 ZIP、CLI、PSD/PNG 样本的 SHA-256，不启动 YuTool。
4. 再执行：
   `bash First-User-Test.command --run`
   它会在**全新的隔离测试目录**中自动运行版本、健康检查、图片缩放、引擎安装、
   PSD 读取、图层导出、引擎卸载，不接触你的个人 PSD 或原有 YuTool 配置。
5. 按照 `MANUAL-ACCEPTANCE.md` 记录真实首次下载/解压/安全弹窗与使用体验。
   自动程序无法代替你对 macOS Gatekeeper 提示和交互复杂度的判断。

如果不会把终端切换到测试目录：在访达中打开测试目录，
将 `First-User-Test.command` 拖进终端，**在命令前手动添加 `bash `**，
末尾添加 ` --verify-only` 或 ` --run`。

## 安全边界

- 不使用 `sudo`，不清除下载隔离标记，不全局关闭 Gatekeeper。
- 如果 macOS 安全检查拒绝首次运行，请先记录**原始提示内容**；
  在确认文件来源与 SHA-256 后，才使用「系统设置 → 隐私与安全性」
  中的单项允许操作。安全提示本身属于验收结果。
- 本包未经过 Apple 公证；如果系统阻止了未公证可执行文件，记录为阻塞，不是假装通过。
- 脚本需要的工具为 macOS 自带的 `bash`、`ditto`、`shasum` 等，
  **不需要安装 Python、Node、Rust、Xcode/Cargo**。
- 源测试图片和 PSD 来自项目固定测试样本，无个人数据；PSD 只读，不进行图层修改。
- 所有工作目录在当前用户的临时目录中生成，新日志目录只保留这次测试内容。
- 失败时查看终端打印的 `yu-first-user-...` 路径中的 `report.tsv` 与 `logs`。
  共享日志前检查是否包含你的本机路径，不要附带任何私人设计文件或凭证。

**设备验收状态默认 PENDING。只有真的在 MacBook Air 上操作后才能手工记录 PASS。**
"""

MANUAL="""# MacBook Air First User Acceptance — 手工验收

**本表是待填写的记录，全部默认未完成。不要将开发 Mac 上的自动测试当作 Air 实测。**

- [ ] 实际测试设备为 MacBook Air M2（或说明其他设备与系统版本）
- [ ] 从另一台机器收到完整测试 KIT（记录传输方式）
- [ ] 解压 KIT，无需额外安装 SDK / Python / Node / Cargo
- [ ] 首次运行前，记录 macOS 版本及 Gatekeeper 是否启用
- [ ] 记录实际出现的「无法验证开发者」/安全警告（如有），不全局关闭安全措施
- [ ] `--verify-only` 正常完成
- [ ] `--run` 的 Version / Doctor / 图片读取和缩放成功
- [ ] 可选 ag-psd 引擎安装、激活、PSD 读取、图层导出成功
- [ ] 引擎卸载成功，未修改既有用户数据或测试来源文件
- [ ] 文件操作、错误提示和流程能让普通用户理解

设备型号：
系统版本：
文件来源及 SHA256：
安全提示原文：
第一次操作最难理解的步骤：
自动测试 report.tsv 路径：
程序执行结果：PASS / FAIL / BLOCKED / PENDING
记录问题及复现操作：
是否建议进入 beta.1：待决定

## 问题优先级

P0：数据覆盖、泄露、安全绕过、安装损坏；立即阻止下一批测试。
P1：用户无法启动、无法安装引擎、功能核心路径失败。
P2：双层 ZIP/命令过多、报错不明确、需要开发知识。
P3：说明文字、文件命名等体验改善。

**记录 PASS 需要真实 Air 运行证据；先完成真实操作，再根据证据更新。**
"""


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def regular(file: Path, limit: int=80*1024*1024) -> bytes:
    require(file.is_file() and not file.is_symlink(), "missing or symlink source: "+str(file))
    require(0<file.stat().st_size<=limit, "unbounded source file")
    return file.read_bytes()


def entries(private_preview: Path, repository: Path) -> dict[str,bytes]:
    source=private_preview/MAC_ZIP
    data=regular(source)
    require(sha(data)==MAC_SHA, "frozen Mac bundle SHA mismatch")
    psd=regular(repository/"fixtures/psd/upstream/psd-tools/2layers.psd",10*1024*1024)
    require(sha(psd)==PSD_SHA, "fixed PSD fixture SHA mismatch")
    png=tiny_png()
    require(sha(png)==PNG_SHA, "fixed PNG fixture SHA mismatch")
    script=regular(repository/"tools/first_user_mac_acceptance.sh",200_000)
    require(b"spctl --master-disable" not in script and b"sudo " not in script and
            b"xattr -d " not in script, "script contains disallowed Gatekeeper bypass")
    require(script.startswith(b"#!/bin/bash"), "unexpected test script format")
    return {
        "First-User-Test.command":script,
        MAC_ZIP:data,
        "README-FIRST.zh-CN.md":QUICKSTART.encode(),
        "MANUAL-ACCEPTANCE.md":MANUAL.encode(),
        "test-fixtures/2layers.psd":psd,
        "test-fixtures/sample.png":png,
    }


def build(preview: Path, repository: Path, dest: Path) -> dict:
    # Verifies all three unchanged source bundles before copying one to Air.
    original=verify_preview(preview,repository)
    require(original["status"]=="PASS" and original["bundles"]==3,
            "source private preview is not accepted")
    payload=entries(preview,repository)
    require(not dest.exists() and not dest.is_symlink(), "kit destination already exists")
    require(dest.parent.is_dir() and not dest.parent.is_symlink(), "kit output parent invalid")
    dest.mkdir(mode=0o700)
    folder=dest/"YuTool-First-User-Mac"
    folder.mkdir(mode=0o700)
    record={"schema_version":"1","milestone":"v0.1-dev First User Acceptance",
            "frozen_main_sha":SOURCE,"platform":"macos-arm64",
            "automated_macbook_air_real_first_run":"PENDING",
            "manual_gatekeeper_acceptance":"PENDING",
            "public_release_authorized":False,"apple_notarization_performed":False,
            "psd_mutation_available":False,
            "source_bundle_sha256":MAC_SHA,
            "files_sha256":{name:sha(data) for name,data in payload.items()}}
    payload["KIT-MANIFEST.json"]=json.dumps(record,ensure_ascii=False,
                                           sort_keys=True,indent=2).encode()+b"\n"
    checksums="".join(f"{sha(data)}  {name}\n"
                     for name,data in sorted(payload.items()))
    payload["SHA256SUMS"]=checksums.encode()
    for name,data in payload.items():
        file=folder/name
        file.parent.mkdir(parents=True,exist_ok=True)
        with file.open("xb") as stream:stream.write(data)
        if name=="First-User-Test.command":file.chmod(0o700)
    zipout=dest/f"{LABEL}-macos-arm64.zip"
    with zipfile.ZipFile(zipout,"x",compression=zipfile.ZIP_STORED) as z:
        for name,data in sorted(payload.items()):
            z.writestr("YuTool-First-User-Mac/"+name,data)
    digest=sha(zipout.read_bytes())
    (dest/(zipout.name+".sha256")).write_text(f"{digest}  {zipout.name}\n")
    return {"status":"BUILT_AND_UNTESTED_ON_AIR","file":zipout.name,"bytes":zipout.stat().st_size,
            "sha256":digest,"source_sha256":MAC_SHA,"air_gate":"PENDING"}


def verify(dest: Path) -> dict:
    files=list(dest.glob(f"{LABEL}*-macos-arm64.zip"))
    require(len(files)==1,"exactly one MacBook Air kit expected")
    zipout=files[0]
    data=regular(zipout,120*1024*1024)
    require((dest/(zipout.name+".sha256")).read_text()==
            f"{sha(data)}  {zipout.name}\n","outer kit SHA256SUMS changed")
    with zipfile.ZipFile(zipout) as z:
        names=z.namelist()
        require(len(names)==len(set(names)) and len(names)==8,
                "invalid or incomplete First User kit layout")
        require(all(name.startswith("YuTool-First-User-Mac/") and
                    ".." not in Path(name).parts for name in names),
                "kit contains unsafe zip paths")
        record=json.loads(z.read("YuTool-First-User-Mac/KIT-MANIFEST.json"))
        require(record["frozen_main_sha"]==SOURCE and
                record["automated_macbook_air_real_first_run"]=="PENDING" and
                record["manual_gatekeeper_acceptance"]=="PENDING" and
                record["public_release_authorized"] is False,"unapproved FUA claim")
        for name,expected in record["files_sha256"].items():
            require(sha(z.read("YuTool-First-User-Mac/"+name))==expected,
                    "kit payload changed: "+name)
        require(sha(z.read("YuTool-First-User-Mac/"+MAC_ZIP))==MAC_SHA,
                "nested original macOS ZIP changed")
    return {"status":"PASS","zip":zipout.name,"sha256":sha(data),
            "checked_members":len(names),"air_first_user_acceptance":"PENDING",
            "no_Gatekeeper_bypass":True}


def main()->None:
    p=argparse.ArgumentParser()
    p.add_argument("--preview-dir",type=Path)
    p.add_argument("--repository",type=Path,default=Path.cwd())
    p.add_argument("--output-dir",type=Path,required=True)
    p.add_argument("--verify-only",action="store_true")
    args=p.parse_args()
    if not args.verify_only:
        require(args.preview_dir is not None,"--preview-dir is required to create a kit")
        result=build(args.preview_dir.resolve(),args.repository.resolve(),
                     args.output_dir.resolve())
        print(json.dumps(result,ensure_ascii=False))
    print(json.dumps(verify(args.output_dir.resolve()),ensure_ascii=False))


if __name__=="__main__":
    try:main()
    except (KeyError,ValueError,OSError,zipfile.BadZipFile) as err:
        sys.exit("FIRST_USER_MAC_KIT_REFUSED: "+str(err))
