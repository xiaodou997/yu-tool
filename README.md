# YuTool

> **One CLI. Many capabilities.**

YuTool is a lightweight, extensible local tool runtime for developers, automation, and AI agents.

Instead of requiring users or agents to learn and manage many unrelated command-line tools, YuTool exposes local capabilities through one consistent CLI:

\`\`\`bash
yu doctor
yu capabilities
yu engine list

yu image info photo.jpg
yu image resize photo.jpg --width 1024

yu psd inspect design.psd
yu psd tree design.psd --json
\`\`\`

The project is intentionally **engine-oriented**. YuTool keeps the core small and adds capabilities through built-in, managed, or system engines.

## Why YuTool?

Many useful local operations already have excellent implementations: image processing, PSD parsing, PDF tooling, media conversion, metadata extraction, OCR, and more. The problem for developers and agents is often not the lack of tools, but the lack of a stable, discoverable interface across them.

YuTool aims to provide that interface.

- **Lightweight** — keep the core small; install optional engines only when needed.
- **Unified** — one command model, one structured output model, one error model.
- **Extensible** — combine native Rust engines with mature external tools and runtimes.
- **Agent-friendly** — JSON output, capability discovery, predictable exit codes, dry-run support, and non-destructive defaults.
- **Local-first** — capabilities run on the user's machine unless a future feature explicitly states otherwise.

## Brand

**Yu** comes from the Chinese character **羽** — feather.

A feather is light, but its structure is composed of many parts working together. YuTool follows the same idea: a small core with capabilities attached only when needed.

**Small core. Flexible engines. One interface.**

## Project naming

| Surface | Name |
| --- | --- |
| Brand | **YuTool** |
| Repository | **yu-tool** |
| CLI | **yu** |
| Desktop manager | **YuTool Manager** |

## Architecture at a glance

\`\`\`text
Developers / Scripts / AI Agents
              │
              ▼
             yu
              │
       Capability Registry
              │
       Engine Resolver
              │
   ┌──────────┼──────────┐
   ▼          ▼          ▼
Built-in    Managed     System
   │          │          │
 Rust      downloaded   existing
engines      engines     tools
\`\`\`

An engine can be:

- **Built-in** — compiled into YuTool and available immediately.
- **Managed** — installed, updated, and removed by YuTool.
- **System** — already available on the host machine and discovered by YuTool.

The CLI should prefer a suitable built-in engine by default, while allowing users and agents to inspect or explicitly select another engine when needed.

## Initial scope

YuTool starts with **image and layered-image workflows** because they provide a useful test bed for the engine model.

Initial capability areas include:

- raster image inspection and transformation;
- PSD/PSB inspection and layer discovery;
- rendering and export workflows;
- engine discovery and health checks;
- machine-readable JSON output.

Potential engines include native Rust implementations as well as tools such as psd-tools, ImageMagick, libvips, ExifTool, and others. An engine is not considered a hard dependency merely because YuTool can integrate with it.

Future capability areas may include PDF, media, metadata, OCR, archives, and document conversion. These are explicitly future scope and should not inflate the first release.

## CLI principles

The public CLI is treated as a product API.

\`\`\`bash
yu doctor
yu capabilities --json

yu engine list
yu engine info <engine>
yu engine install --manifest ./engine.json
yu engine versions <engine>
yu engine activate <engine> <version>
yu engine deactivate <engine>
yu engine remove <engine> <version>

yu image info <file>
yu image resize <file> --width 1024 -o output.jpg

yu psd inspect <file>
yu psd tree <file> --json
yu psd layer list <file> --json
\`\`\`

Core commands should support structured output where it is useful to automation.

The intended agent workflow is:

\`\`\`text
discover capabilities
        ↓
inspect input
        ↓
plan
        ↓
dry-run when supported
        ↓
apply
        ↓
render / validate
        ↓
report structured result
\`\`\`

## Current status

YuTool has completed the **M2 Engine Manager baseline** and is now building the **M3 PSD Engine Spike** harness before selecting a PSD engine.

The current runtime includes the Rust core, the built-in raster engine, verified Managed Engine installation/lifecycle, per-engine mutation locking, and unified Built-in/Managed/System discovery. M3 now has an engine-neutral PSD fixture/conformance harness with three wired candidates: psd-tools 1.20.0 (7/7 corpus v1), rawpsd 0.2.2 (4/7), and ag-psd 31.0.2 (7/7). The comparison report records conformance and distribution evidence, but no PSD engine has been selected yet. The built-in raster scope remains intentionally small: PNG, JPEG, and WebP.

See:

- [Vision](docs/vision.md)
- [Architecture](docs/architecture.md)
- [CLI specification](docs/cli-spec.md)
- [Capability matrix](docs/capabilities.md)
- [Agent guide](docs/agent-guide.md)
- [Roadmap](docs/roadmap.md)
- [PSD candidate comparison](docs/psd-candidate-comparison.md)
- [PSD controlled benchmark](docs/psd-controlled-benchmark-report.md)
- [Engine strategy ADR](docs/decisions/0001-engine-strategy.md)

## 中文简介

**YuTool（羽）是一个面向开发者、自动化脚本与 AI Agent 的轻量、可扩展本地工具运行时。**

YuTool 不要求使用者分别学习和管理大量底层工具，而是通过统一的 \`yu\` 命令暴露本地能力。

核心保持轻量；复杂能力通过 Engine 按需组合和安装。

一句话：

> **一个 \`yu\`，调用所需能力。**
