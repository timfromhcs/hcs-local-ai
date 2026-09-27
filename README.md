# HCS Local AI

[![Release](https://img.shields.io/badge/release-v1.0.0-blue.svg)](https://github.com/timfromhcs/hcs-local-ai)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64%20|%20Linux%20x64-lightgrey.svg)]()
[![Hardware](https://img.shields.io/badge/Vulkan-AMD%20iGPU%20|%20Unified%20Memory-orange.svg)]()
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green.svg)]()

> **HCS Local AI** is a production-grade, local-first AI serving stack and autonomous orchestration daemon. It exposes standard OpenAI-compatible and Anthropic-compatible APIs, an official OpenJev decision contract endpoint, an autonomous agentic execution engine, persistent SQLite WAL memory, and an integrated real-time web dashboard—all natively executing on **AMD iGPU / Vulkan** within a constrained 20–24 GB unified memory footprint.

---

## 🏛️ System Architecture

```text
                               +----------------------------------------+
                               |        HCS Dashboard & Clients         |
                               | (OpenAI / Anthropic / Browser SPA SSE) |
                               +-------------------+--------------------+
                                                   | HTTP :8787
                                                   v
+---------------------------------------------------------------------------------------------------+
|                                        hcs-daemon.exe                                             |
|                                                                                                   |
|  +--------------------+  +----------------------+  +---------------------+  +------------------+  |
|  |   OpenAI Adapter   |  |  Anthropic Adapter   |  |   HCS Native API    |  | Web UI & SSE Srv |  |
|  +---------+----------+  +----------+-----------+  +----------+----------+  +--------+---------+  |
|            |                        |                         |                      |            |
|            +------------------------+------------+------------+----------------------+            |
|                                                  |                                                |
|                                                  v                                                |
|  +---------------------------------------------------------------------------------------------+  |
|  |                                  Unified Model Orchestrator                                 |  |
|  |  - Model Lifecycle (Cold -> Validating -> Loading -> Ready -> Active -> Quarantined)        |  |
|  |  - Memory-Aware Scheduler (max_heavy_active = 1, eviction safety)                           |  |
|  |  - Autonomous Agent & Tool Engine (file_read, file_write, file_edit, command_exec)          |  |
|  |  - OpenJev Decision Contract Evaluator (A-P candidate mapping, temp=0)                     |  |
|  |  - Watchdog & Self-Healing Engine (crash auto-respawn, quarantine after 3 crashes)          |  |
|  |  - Telemetry & Metrics (real-time tok/s, latency histograms, SQLite request traces)         |  |
|  +-------------------------------+----------------------------------+--------------------------+  |
+----------------------------------|----------------------------------|-----------------------------+
                                   |                                  |
                                   v                                  v
                +------------------------------------+  +------------------------------------+
                |       Prism / llama-server         |  |        stable-diffusion.cpp        |
                |   Vulkan Backend (AMD Radeon)      |  |    Vulkan Backend (AMD Radeon)     |
                |   -ngl 99 / 32, --flash-attn auto  |  |    Flux.2 Klein Flow Sampler       |
                +------------------------------------+  +------------------------------------+
```

---

## 🚀 Key Features

- **Native Single-Node Daemon**: No Docker, no WSL, no VMs required. Single lightweight Rust binary (`hcs-daemon.exe`).
- **Constrained Unified Memory Awareness**: Operates within 20–24 GB shared memory on AMD iGPU laptops/desktops without out-of-memory kernel panics or swapping stalls. Enforces `max_heavy_active = 1`.
- **Zero Mock Acceptance**: Every backend is backed by real model weights and real Vulkan GPU pipelines.
- **OpenAI & Anthropic Compatibility**: Drop-in compatible with standard OpenAI SDKs and Claude clients.
- **Official OpenJev Decision Contract**: Implements the official `jev.dynamic.prompt.v2` decision protocol for deterministic routing, tool selection, and candidate gating at temperature 0.
- **Autonomous Agentic Software Engineering**: Autonomous multi-turn agent with built-in sandbox tools (`file_read`, `file_write`, `file_edit`, `command_exec`) and self-healing diagnostic error recovery loops.
- **Native Real-Time Web Dashboard**: 12 dedicated views for overview, models, requests, telemetry, agent tasks, memory, API keys, system diagnostics, and live SSE event streams.

---

## 📦 Required Model Roster

| Alias | Upstream Model | Quant | Size | Hardware Target & Offload |
|---|---|---|---|---|
| `hcs-subagent` | `prism-ml/Bonsai-1.7B-gguf` | Q1_0 | 248 MB | 100% GPU (`-ngl 99`), 4k context |
| `hcs-general` | `prism-ml/Ternary-Bonsai-4B-gguf` | PQ2_0 | 1.07 GB | 100% GPU (`-ngl 99`), 8k context |
| `hcs-coder` | `prism-ml/Ternary-Bonsai-2-27B-gguf` | PQ2_0 | 7.20 GB | 100% GPU (`-ngl 99`), 8k context |
| `hcs-judge` | `prithivMLmods/APUS-OpenJev-v1-4B-GGUF` | Q4_K_M | 2.70 GB | 100% GPU (`-ngl 99`), 4k context |
| `hcs-vlm` | `DavidAU/Qwen3.5-9B-The-Defiant-Fable-MAX-GGUF` | Q4_K_M + F16 mmproj | 7.74 GB | Hybrid Vulkan (`-ngl 32`), 4k context |
| `hcs-image` | `Aatricks/bonsai-image-ternary-4B-FLUX2-klein-GGUF` | Q2_K + Qwen3-4B | 4.8 GB | Vulkan0 Flow Euler, 512x512 |

---

## ⚡ Real-World Benchmarks (AMD Radeon Graphics iGPU)

Hardware: AMD Ryzen (16 logical cores), AMD Radeon(TM) Graphics (Vulkan UMA), 20 GB Unified Memory, Windows 11 Pro.

| Task / Model | Prompt Processing | Token Generation / Sampler Speed | Latency |
|---|---|---|---|
| **hcs-subagent** (1.7B) | **386.3 tok/s** | **72.8 tok/s** | 250 ms |
| **hcs-general** (4B) | **210.5 tok/s** | **41.2 tok/s** | 480 ms |
| **hcs-judge** (OpenJev 4B) | N/A | Deterministic Decision Label | **243 ms** |
| **hcs-vlm** (9B + Vision) | **41.9 tok/s** | **8.1 tok/s** | Multimodal Verified |
| **hcs-image** (FLUX.2 T2I) | N/A | 4 sampling steps | **133.9 s** |
| **hcs-image** (FLUX.2 I2I) | N/A | 3 sampling steps (strength 0.6) | **104.0 s** |

---

## 🛠️ CLI Quickstart

### 1. Run Doctor Diagnostics
Verify hardware capabilities, Vulkan drivers, runtime binaries, model manifests, and SQLite integrity:
```powershell
.\hcs-daemon.exe doctor
```
```text
=== HCS Local AI Doctor Diagnostics ===
Overall Status: OK

[OK] Operating System: Detected windows on x86_64
[OK] Memory: Total 20261 MB, Available 12540 MB
[OK] Prism Vulkan Runtime: Validated llama-server.exe
[OK] Stable Diffusion Vulkan Runtime: Validated sd-cli.exe
[OK] Model hcs-subagent: Present and validated
[OK] Model hcs-general: Present and validated
[OK] Model hcs-coder: Present and validated
[OK] Model hcs-judge: Present and validated
[OK] Model hcs-vlm: Present and validated
[OK] Model hcs-image: Present and validated
[OK] SQLite Database: Database schema is valid and initialized
```

### 2. Start the Daemon
```powershell
.\hcs-daemon.exe run
```
Open your browser to:
- **Web Dashboard**: `http://127.0.0.1:8787/`
- **OpenAI API**: `http://127.0.0.1:8787/v1`
- **Anthropic API**: `http://127.0.0.1:8787/v1/messages`
- **HCS Native API**: `http://127.0.0.1:8787/hcs/v1`

---

## 🔌 API Compatibility

### OpenAI Compatibility (`/v1`)
- `POST /v1/chat/completions`: Full streaming (`text/event-stream`) and non-streaming, multi-role (`system`, `user`, `assistant`, `tool`), tool calling (`tools`, `tool_choice`).
- `GET /v1/models`: Enumerates all active models with quantization and capabilities.
- `POST /v1/images/generations`: Text-to-Image generation using FLUX.2 Klein on Vulkan.
- `POST /v1/images/edits`: Image-to-Image transformation and editing using FLUX.2 Klein on Vulkan.
- `POST /v1/files` & `GET /v1/files`: Local file storage and artifact indexing.
- `POST /v1/batches`: Asynchronous batch processing queue.

### Anthropic Compatibility (`/v1/messages`)
- `POST /v1/messages`: Claude-compatible content blocks (`text`, `image`), streaming SSE translation (`message_start`, `content_block_delta`, `message_stop`), `max_tokens`, `temperature`.
- `POST /v1/messages/count_tokens`: Token counting endpoint.

### OpenJev Decision Protocol (`POST /hcs/v1/decision`)
```json
{
  "state": "Unit tests failed with code 1 in auth module",
  "instructions": "Select the highest-priority diagnostic action",
  "candidates": [
    {"id": "inspect_test", "description": "Read test failure log and inspect assertion diff"},
    {"id": "recompile", "description": "Rebuild target without modifications"},
    {"id": "abort", "description": "Abort test execution"}
  ]
}
```
Response:
```json
{
  "selected_id": "inspect_test",
  "selected_label": "A",
  "confidence": 0.95,
  "raw_output": "A"
}
```

### Autonomous Agent Runtime (`POST /hcs/v1/agent/run`)
```json
{
  "prompt": "Create a file named verified.txt and verify its contents using tools.",
  "model": "hcs-coder"
}
```

---

## 📜 License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
