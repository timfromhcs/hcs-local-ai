# HCS Local AI

[![Release](https://img.shields.io/badge/release-v1.0.0-blue.svg)](https://github.com/timfromhcs/hcs-local-ai)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64%20|%20Linux%20x64-lightgrey.svg)]()
[![Hardware](https://img.shields.io/badge/Vulkan-AMD%20iGPU%20|%20Unified%20Memory-orange.svg)]()
[![Tests](https://img.shields.io/badge/tests-27%2F27%20passed%20(100%25)-brightgreen.svg)]()
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

## 🧪 Comprehensive Verification & Stress Test Scorecard

### 1. Native Unit Tests (`cargo test`)
**11 / 11 PASSED (100%)** — 0 warnings, 0 errors.

| Module | Test Name | Description |
|---|---|---|
| `agent::self_healing` | `test_self_healing_diagnosis_and_reporting` | Diagnoses tool exit codes, generates structured healing reports, tracks status |
| `agent::tools` | `test_tool_executor_files_and_command` | Sandboxed `file_write`, `file_read`, `file_edit`, and `command_exec` |
| `openjev` | `test_openjev_validation` | Candidate boundary enforcement (2–16 candidates), strict schema checks |
| `openjev` | `test_openjev_rendering_and_parsing` | Prompt rendering with `PROMPT_VERSION = "jev.dynamic.prompt.v2"` and strict A–P label parsing |
| `config` | `test_config_defaults_and_save` | Config defaults, path expansion, and serialization |
| `db` | `test_db_api_key_lifecycle` | API key generation, prefix hashing, permission validation, revocation |
| `db` | `test_db_memory_search` | Persistent SQLite WAL memory storage, categories, and keyword search |
| `db` | `test_db_jobs_and_audit` | Batch job status state transitions and audit logging |
| `models` | `test_model_alias_resolution` | Fast alias resolution across all 6 model tiers |
| `models` | `test_model_lifecycle_transitions` | State machine: `Cold -> Validating -> Loading -> Ready -> Active -> Quarantined` |
| `resource` | `test_resource_manager_heavy_concurrency` | Semaphore enforcement (`max_heavy_active = 1`) ensuring memory safety |

### 2. End-to-End & Concurrency Stress Test Suite (`python tests/stress_test.py`)
**27 / 27 CHECKS PASSED (100% SUCCESS)** against the production release binary.

| Category | Endpoint / Action | Verified Behavior | Status |
|---|---|---|:---:|
| System Diagnostics | `GET /hcs/v1/system` | System health, UMA memory readings, uptime | **PASS** |
| Dashboard UI | `GET /` | Responsive single-page dashboard HTML (12 views) | **PASS** |
| Model Discovery | `GET /v1/models` & `GET /hcs/v1/models` | All 6 models enumerated with parameters & quants | **PASS** |
| Security & Auth | `POST /hcs/v1/keys` & `GET /hcs/v1/keys` | API key issuance, prefix indexing, permissions | **PASS** |
| Persistent Memory | `POST/GET/DELETE /hcs/v1/memory` | SQLite CRUD, categorization, and search | **PASS** |
| OpenAI Chat | `POST /v1/chat/completions` (JSON) | Non-streaming completion with latency < 300ms | **PASS** |
| OpenAI Streaming | `POST /v1/chat/completions` (SSE) | Real-time `text/event-stream` chunks + `[DONE]` | **PASS** |
| Anthropic Messages | `POST /v1/messages` | Claude format content blocks (`text`) | **PASS** |
| Anthropic Tokens | `POST /v1/messages/count_tokens` | Input token counting | **PASS** |
| OpenJev Decision | `POST /hcs/v1/decision` | Deterministic label resolution (`A` -> candidate ID) | **PASS** |
| Files API | `POST /v1/files`, `GET /v1/files/{id}`, `GET /v1/files/{id}/content`, `DELETE` | Upload, metadata, binary download, and deletion | **PASS** |
| Batch Processing | `POST /v1/batches` & `GET /v1/batches/{id}` | Asynchronous job creation and status tracking | **PASS** |
| Autonomous Agent | `POST /hcs/v1/agent/run` | Multi-turn tool execution loop (`command_exec`, `file_read`, etc.) | **PASS** |
| Telemetry & Tracing | `GET /hcs/v1/telemetry` & `GET /hcs/v1/requests` | Real-time counters, throughput, SQLite traces | **PASS** |
| Fault Injection | Malformed JSON payload | Graceful `400 Bad Request` rejection | **PASS** |
| Fault Injection | Invalid route | Clean `404 Not Found` response | **PASS** |
| **Concurrency Stress** | **8 Simultaneous Inferences** | **8 parallel requests handled concurrently, 0 failures, avg latency 0.88s** | **PASS** |
| Model Lifecycle | `POST /hcs/v1/models/{id}/unload` | Safe graceful worker unload and memory release | **PASS** |

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

## 🔌 API Reference Matrix

| Method | Endpoint | Description | Protocol |
|---|---|---|---|
| `GET` | `/` | Web Dashboard Single Page Application | HTML / SSE |
| `GET` | `/v1/models` | List all available models and capabilities | OpenAI |
| `POST` | `/v1/chat/completions` | Multi-turn chat completions (streaming & non-streaming) | OpenAI |
| `POST` | `/v1/responses` | Agentic unified responses protocol | OpenAI |
| `POST` | `/v1/images/generations` | Text-to-Image synthesis (FLUX.2 Klein Vulkan) | OpenAI |
| `POST` | `/v1/images/edits` | Image-to-Image transformation (FLUX.2 Klein Vulkan) | OpenAI |
| `POST` | `/v1/files` | Upload and store local artifacts | OpenAI |
| `GET` | `/v1/files` | List stored artifacts | OpenAI |
| `GET` | `/v1/files/{id}` | Retrieve file metadata | OpenAI |
| `GET` | `/v1/files/{id}/content` | Download binary file content | OpenAI |
| `DELETE`| `/v1/files/{id}` | Delete stored file | OpenAI |
| `POST` | `/v1/batches` | Create asynchronous batch inference job | OpenAI |
| `GET` | `/v1/batches/{id}` | Inspect batch job execution status | OpenAI |
| `POST` | `/v1/messages` | Claude-compatible chat messages (streaming & blocks) | Anthropic |
| `POST` | `/v1/messages/count_tokens` | Token count calculation | Anthropic |
| `GET` | `/hcs/v1/system` | Hardware, memory, and daemon status | HCS Native |
| `GET` | `/hcs/v1/doctor` | Comprehensive system self-check | HCS Native |
| `GET` | `/hcs/v1/models` | HCS model registry and load state | HCS Native |
| `POST` | `/hcs/v1/models/{id}/load` | Explicitly warm and load model into Vulkan | HCS Native |
| `POST` | `/hcs/v1/models/{id}/unload` | Explicitly unload model to reclaim memory | HCS Native |
| `POST` | `/hcs/v1/decision` | OpenJev deterministic decision contract | HCS Native |
| `POST` | `/hcs/v1/agent/run` | Execute autonomous agent with sandboxed tools | HCS Native |
| `GET` | `/hcs/v1/memory` | Search persistent SQLite memory | HCS Native |
| `POST` | `/hcs/v1/memory` | Insert persistent SQLite memory item | HCS Native |
| `DELETE`| `/hcs/v1/memory/{id}` | Remove memory item | HCS Native |
| `GET` | `/hcs/v1/keys` | List active API keys | HCS Native |
| `POST` | `/hcs/v1/keys` | Generate new API key with permissions | HCS Native |
| `GET` | `/hcs/v1/telemetry` | Real-time performance counters and tok/s | HCS Native |
| `GET` | `/hcs/v1/requests` | Historical request traces from SQLite | HCS Native |
| `GET` | `/hcs/v1/events` | Real-time Server-Sent Events (SSE) stream | HCS Native |

---

## 🎯 OpenJev Decision Protocol (`POST /hcs/v1/decision`)

OpenJev evaluates candidate choices deterministically at `temperature = 0`:

```json
{
  "state": "Unit tests failed with code 1 in auth module",
  "instructions": "Select the highest-priority diagnostic action",
  "criteria": [
    {"id": "inspect_test", "description": "Read test failure log and inspect assertion diff"},
    {"id": "recompile", "description": "Rebuild target without modifications"},
    {"id": "abort", "description": "Abort test execution"}
  ]
}
```
Response:
```json
{
  "id": "67b93198-42f0-4a8b-a45e-4efbb8e84cf9",
  "selected_id": "inspect_test",
  "selected_label": "A",
  "confidence": 0.95,
  "raw_output": "A"
}
```

---

## 🤖 Autonomous Agent Runtime (`POST /hcs/v1/agent/run`)

The autonomous agent executes multi-turn tool loops with built-in sandboxing and self-healing:
- `file_read`: Read target files with optional line offsets.
- `file_write`: Atomic file creation and updates.
- `file_edit`: Targeted line-based text replacements.
- `command_exec`: Sandboxed shell execution with timeout and output capture.

```json
{
  "prompt": "Inspect src/config.rs, add a timeout field with default 30s, and run cargo test.",
  "model": "hcs-coder"
}
```

---

## 🏃 Running the Tests

### Native Rust Unit Tests
```powershell
cargo test
```

### End-to-End Integration & Stress Test
Start the daemon in one terminal:
```powershell
.\target\release\hcs-daemon.exe run
```
Run the automated test suite in another terminal:
```powershell
python tests/stress_test.py
```

---

## 📜 License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))
