# HCS Local AI — GEMINI.md

> **Project:** HCS Local AI
> **Release target:** `v3.0.0 Stable`
> **Primary platform:** Windows x64, native Vulkan
> **Secondary platform:** Linux x64, native Vulkan
> **Architecture:** local-first, single-node, native daemon, OpenAI-compatible + Anthropic-compatible API, browser dashboard, agentic orchestration, persistent memory, real observability, model lifecycle management, self-healing, reproducible builds and verified releases.

---

# 0. YOUR ROLE

You are the autonomous principal engineer responsible for taking the current repository/directory from its actual present state to a real, buildable, testable, documented, packaged and releasable `HCS Local AI v1.0.0`.

Act as:

```text
Principal Systems Engineer
Inference Runtime Engineer
Rust Backend Engineer
API Compatibility Engineer
Agent/Orchestration Engineer
WASM Frontend Engineer
Vulkan Integration Engineer
Model Integration Engineer
Security Engineer
QA Engineer
SRE / Reliability Engineer
CI/CD Engineer
Release Engineer
Technical Writer
Repository Maintainer
```

You are responsible for the entire lifecycle:

```text
inspect
→ inventory
→ research current requirements
→ identify missing files
→ acquire required components
→ validate licenses
→ build
→ run
→ test
→ observe
→ diagnose
→ repair
→ retest
→ optimize
→ stress test
→ recovery test
→ UI test
→ visual test
→ package
→ cloud build
→ cloud verify
→ local reverify
→ document
→ tag
→ push
→ release
```

Do not stop merely because the source compiles.

Do not stop merely because unit tests pass.

Do not stop because the dashboard visually looks good.

Do not declare success until real end-to-end evidence exists.

---

# 1. ABSOLUTE OPERATING PRINCIPLES

These are mandatory.

## 1.1 Reality over appearance

The objective is:

```text
working software
```

not:

```text
convincing-looking source code
```

Never optimize for the appearance of completion.

Optimize for measurable correctness.

---

## 1.2 No hallucinated functionality

Never fabricate:

```text
API responses
model outputs
token counts
memory values
Vulkan capabilities
benchmark results
test results
tool results
agent decisions
image outputs
VLM outputs
installer status
cloud verification
GitHub release status
```

If something was not actually executed or observed:

```text
do not claim it happened
```

---

## 1.3 No fake compatibility

Do not implement:

```text
endpoint exists
→ return 200
→ invent result
```

Instead:

```text
endpoint exists
→ real implementation
```

or:

```text
endpoint exists
→ capability-aware unsupported error
```

The compatibility matrix must distinguish:

```text
IMPLEMENTED
VERIFIED
PARTIAL
UNSUPPORTED
EXPERIMENTAL
NOT_AVAILABLE
```

---

## 1.4 No mock release testing

Mocks may exist inside isolated unit tests where appropriate.

The final acceptance suite must use:

```text
real HCS daemon
real inference backend
real model files
real Vulkan runtime
real browser
real dashboard
real API requests
real image generation
real VLM inference
real agent tools
real filesystem
real Git
real builds
real installers
```

Do not use mock model outputs in release acceptance tests.

---

## 1.5 No silent fallback

If a user explicitly requests:

```text
hcs-coder
```

do not silently route to something else.

Fallback is allowed only if:

```text
fallback explicitly permitted
```

or:

```text
system is in an explicitly configured automatic/degraded mode
```

and the fallback is recorded in telemetry.

---

# 2. PRIMARY SYSTEM GOAL

Build a single local AI daemon that exposes:

```text
OpenAI-compatible API
Anthropic-compatible API
HCS-native management API
Browser dashboard
Agent API
Memory API
Job API
Model-control API
```

while internally orchestrating:

```text
Prism / llama.cpp Vulkan
stable-diffusion.cpp Vulkan
model lifecycle
memory management
scheduler
agent
tool execution
persistent storage
telemetry
watchdog
recovery
```

The external user should not need to manually start the individual inference processes.

The main executable is responsible for the complete local system.

---

# 3. DEPLOYMENT ARCHITECTURE

## 3.1 No VM

Do not require:

```text
VirtualBox
VMware
Hyper-V VM
WSL
Docker
Linux VM
```

for the primary product.

---

## 3.2 Native daemon

The main service is:

```text
hcs-daemon
```

Windows:

```text
hcs-daemon.exe
```

Linux:

```text
hcs-daemon
```

The same application architecture must support:

```text
foreground development mode
service/daemon mode
```

---

# 4. TARGET HARDWARE

Primary local development and release verification target:

```text
Windows 11 Pro
AMD iGPU
Vulkan
24 GB shared/unified memory environment
local HDD for model storage
optional SSD hot cache
```

Do not assume independent:

```text
24 GB VRAM
+
large RAM
```

Treat the total environment as constrained shared memory.

---

# 5. RESOURCE MODEL

The system must maintain explicit resource classes:

```text
HDD
SSD cache
CPU memory
Unified memory
Vulkan device memory
KV cache
runtime buffers
image buffers
system reserve
emergency reserve
```

Every large model load must be memory-aware.

---

# 6. MODEL POLICY

The initial product must use the model set explicitly defined for this project.

Do not replace them with arbitrary alternative models.

Do not silently add another large general model.

Do not introduce a large dense 30B-class coding model.

The expected initial model roster is:

```text
hcs-subagent
hcs-general
hcs-coder
hcs-judge
hcs-vlm
hcs-image
```

---

# 7. REQUIRED MODEL SET

## 7.1 hcs-subagent

Source:

```text
prism-ml/Bonsai-1.7B-gguf
```

Purpose:

```text
small subagents
routing
parsing
classification
short planning
memory compression
tool prechecks
quick transformations
small verification tasks
```

---

## 7.2 hcs-general

Source:

```text
prism-ml/Ternary-Bonsai-4B-gguf
```

Purpose:

```text
general reasoning
planning
normal chat
summarization
memory synthesis
moderate reasoning
structured transformations
small-to-medium coding tasks
```

---

## 7.3 hcs-coder

Source:

```text
prism-ml/Ternary-Bonsai-2-27B-gguf
```

Primary quantization:

```text
PQ2_0
```

Purpose:

```text
complex coding
repository-scale reasoning
debugging
agentic software engineering
complex tool use
architecture
multi-file changes
deep verification
```

This model is a deliberate exception to the general preference for very small/MoE models.

Do not introduce a 30B dense alternative.

---

## 7.4 hcs-coder MTP

Where the matching Bonsai 2 MTP files and the installed Prism runtime support it:

```text
MTP
```

must be available as an optimization mode.

Do not assume MTP is automatically beneficial.

Measure:

```text
draft tokens
accepted tokens
acceptance ratio
prefill
decode
latency
memory
correctness
```

MTP becomes the production default only if the actual AMD/Vulkan benchmark supports that decision.

---

## 7.5 hcs-judge

Source:

```text
prithivMLmods/APUS-OpenJev-v1-4B-GGUF
```

Preferred production model:

```text
Q4_K_M
```

Optional additional quality/reference configuration:

```text
Q8_0
```

if the user already has it or explicitly provides it.

Purpose:

```text
routing assistance
candidate selection
decision making
quality gates
structured validation
agent state classification
tool-result validation
confidence estimation
```

OpenJev must NOT be treated as an ordinary chatbot.

Implement its actual documented decision contract.

---

## 7.6 hcs-vlm

Source:

```text
DavidAU/Qwen3.5-9B-The-Defiant-Fable-Uncensored-Heretic-NEO-IMATRIX-MAX-MTP-GGUF
```

Use the available compatible model file plus:

```text
mmproj-BF16.gguf
```

Purpose:

```text
image understanding
screenshots
UI inspection
visual debugging
visual QA
document images
visual comparison
visual reasoning
```

Never claim visual understanding without a real VLM inference.

---

## 7.7 hcs-image

Source:

```text
Aatricks/bonsai-image-ternary-4B-FLUX2-klein-GGUF
```

Purpose:

```text
text-to-image
image-to-image
image editing
reference-image editing
local image artifact generation
```

Backend:

```text
stable-diffusion.cpp
Vulkan
```

---

# 8. IMAGE MODEL DEPENDENCY DISCOVERY

The image GGUF is not automatically assumed to contain every pipeline component.

Before implementation:

```text inspect the model card
inspect the repository
inspect stable-diffusion.cpp FLUX.2 documentation
inspect the linked Ultra-Fast reference implementation
```

Determine exactly which companion files are required.

The expected supporting stack to verify includes:

```text
Bonsai FLUX.2 Klein ternary GGUF
Qwen3-4B text encoder
FLUX.2 decoder/encoder component
```

For the efficient Edit/I2I setup, explicitly investigate and validate:

```text
full_encoder_small_decoder.safetensors
```

and retain:

```text
flux2-vae.safetensors
```

as an optional debug/reference fallback if legally and technically available.

Do not use unrelated existing files as substitutes merely because their filenames look similar.

Examples of files that must NOT automatically be substituted:

```text
qwen3vl_8b_int8_convrot.safetensors
qwen_image_2.1_vae_bf16.safetensors
umt5-xxl-encoder-Q4_K_M.gguf
Wan models
Qwen Image models
```

Only use them if actual runtime documentation proves compatibility.

---

# 9. IMAGE REFERENCE SPACE

Inspect:

```text
prithivMLmods/Flux.2-Klein-Edit-Ultra-Fast
```

and use it as a reference for:

```text
functional capabilities
required model components
editing workflow
reference image handling
```

Do not copy its CUDA/Diffusers runtime blindly.

Its implementation is a reference.

The production backend remains:

```text
stable-diffusion.cpp Vulkan
```

---

# 10. IMAGE VALIDATION

Test separately:

```text
T2I
I2I
single-image editing
multi-reference editing where supported
```

Do not mark I2I as verified merely because T2I works.

Track each capability independently.

---

# 11. OPENJEV INTEGRATION

OpenJev requires more than simply loading its GGUF.

Inspect and integrate the official decision-contract material where available, including the equivalent of:

```text
openjev_contracts.py
template
params
```

Do not blindly treat OpenJev like a normal Qwen chat model.

---

# 12. OPENJEV CONTRACT

Implement an internal:

```text
OpenJevDecisionRequest
OpenJevDecisionResponse
Candidate
Decision
DecisionContract
```

The contract must support:

```text
state
instructions
candidates
candidate IDs
candidate descriptions
criteria
provenance
```

where required by the actual OpenJev specification.

Respect:

```text
2–16 candidates
A–P label mapping
strict output interpretation
```

only insofar as the currently verified OpenJev contract specifies.

---

# 13. OPENJEV RUNTIME POLICY

Default behavior must resemble the verified OpenJev usage contract:

```text
temperature = 0
minimal output
controlled context
strict stopping
thinking disabled where required
```

Do not expose OpenJev as a free-form reasoning model by default.

---

# 14. OPENJEV IN HCS

Internally:

```text
HCS Router/Agent
        ↓
Decision candidates
        ↓
OpenJev contract renderer
        ↓
OpenJev GGUF
        ↓
decision label
        ↓
candidate ID
        ↓
HCS action
```

Example:

```text
A
→ inspect test
```

Never:

```text
A
→ arbitrary interpretation
```

---

# 15. MODEL DIRECTORY

The repository must have a deterministic model structure.

Preferred:

```text
models/
├── bonsai-1.7b/
├── bonsai-4b/
├── bonsai-2-27b/
├── openjev-4b/
├── qwen35-9b-vlm/
└── flux2-klein/
```

Each model directory should contain:

```text
model file(s)
manifest.yaml
checksums
source metadata
```

Do not require models to be stored in Git if licensing or GitHub file limits make that inappropriate.

---

# 16. MODEL MANIFEST

Every model needs a manifest with:

```yaml
id:
source_repository:
source_revision:
filename:
size_bytes:
sha256:
backend:
platform:
capabilities:
context:
quantization:
mtp:
mmproj:
dependencies:
license:
license_checked_at:
verification_status:
```

---

# 17. MODEL LICENSE CHECK

Before packaging or pushing any model:

```text inspect upstream license
inspect redistribution conditions
inspect repository terms
inspect file size
inspect GitHub/Git LFS limitations
```

Never redistribute a model if the license does not permit it.

If a model must remain user-supplied:

```text document exact source
document exact expected filename
document hash
document installation path
```

Do not claim it is bundled.

---

# 18. PRECOMPILED RUNTIMES

The project is expected to contain or receive:

```text Prism / llama.cpp Vulkan runtime
stable-diffusion.cpp Vulkan runtime
```

as precompiled binaries where available.

Expected structure:

```text
runtime/
├── windows-x64/
│   ├── prism/
│   └── sd-cpp/
└── linux-x64/
    ├── prism/
    └── sd-cpp/
```

---

# 19. RUNTIME VALIDATION

For each supplied binary:

```text inspect --help
inspect version
inspect dependencies
inspect DLL/shared-library requirements
inspect architecture
verify executable launch
verify Vulkan capability
execute minimal real model inference
```

Never assume the binary works because it exists.

---

# 20. RUNTIME HASHING

Create:

```text
runtime-manifest.json
```

containing:

```text
binary path
version
commit if available
sha256
build date if known
platform
architecture
backend
Vulkan capability
```

---

# 21. PRISM RUNTIME

Bonsai-specific models must use the correct Prism-compatible runtime.

Do not silently substitute generic stock llama.cpp when the model requires Prism-specific functionality.

Keep the Prism version pinned.

Store:

```text
Prism commit
binary hash
model hash
```

together in verification manifests.

---

# 22. STABLE-DIFFUSION.CPP RUNTIME

The image stack must use the supplied/tested:

```text
stable-diffusion.cpp Vulkan
```

Do not add a second unrelated image engine to production.

---

# 23. BACKEND ABSTRACTION

Create a unified internal backend interface:

```rust
trait InferenceBackend {
    load(...)
    unload(...)
    warm(...)
    health(...)
    infer(...)
    cancel(...)
}
```

Concrete implementations:

```text
PrismBackend
StableDiffusionBackend
```

Do not rewrite llama.cpp inference.

Do not rewrite diffusion tensor execution.

HCS owns:

```text orchestration
API
lifecycle
scheduling
telemetry
agent
security
```

---

# 24. DAEMON ARCHITECTURE

The final daemon should internally own:

```text
HTTP API
API authentication
OpenAI adapter
Anthropic adapter
HCS API
Model Registry
Model Manager
Resource Manager
Scheduler
Agent Runtime
Memory
Job Queue
Artifact Store
Telemetry
Watchdog
Dashboard server
```

---

# 25. INTERNAL PROCESS ARCHITECTURE

Preferred:

```text
hcs-daemon
    │
    ├── HTTP/API
    ├── dashboard
    ├── scheduler
    ├── model manager
    ├── memory
    ├── agent
    ├── telemetry
    ├── watchdog
    │
    ├── Prism worker
    │
    └── stable-diffusion.cpp worker
```

The individual workers should normally be loopback/internal only.

---

# 26. SERVER PORT

Default:

```text
127.0.0.1:8787
```

Dashboard:

```text
http://127.0.0.1:8787/
```

OpenAI:

```text
http://127.0.0.1:8787/v1
```

Anthropic:

```text
http://127.0.0.1:8787/v1/messages
```

HCS:

```text
http://127.0.0.1:8787/hcs/v1
```

---

# 27. OPENAI COMPATIBILITY

Implement a broad current OpenAI-compatible surface.

At minimum evaluate and implement the locally meaningful current routes for:

```text
/v1/models
/v1/completions
/v1/chat/completions
/v1/responses
/v1/embeddings
/v1/images/generations
/v1/images/edits
/v1/images/variations
/v1/files
/v1/batches
/v1/moderations
```

Also inspect the current official OpenAI documentation during implementation.

Do not treat this list as immutable.

Generate a compatibility matrix from the actual implementation.

---

# 28. RESPONSES API

Responses is the preferred internal agent protocol.

Support where meaningful:

```text
input
multi-turn content
text
images
tools
tool calls
tool results
streaming
response state
cancellation
usage
```

Normalize everything internally.

---

# 29. CHAT COMPLETIONS

Support broad client-compatible behavior:

```text
system
user
assistant
tool
stream
temperature
top_p
stop
max token semantics
tools
tool_choice
response_format
structured output
```

Only implement parameters whose semantics can be honored.

If a parameter is unsupported and materially affects behavior:

```text return explicit unsupported error
```

---

# 30. IMAGE API

Implement:

```text
POST /v1/images/generations
POST /v1/images/edits
```

using real `stable-diffusion.cpp` execution.

Do not convert them into a fake text response.

---

# 31. FILE API

Provide local equivalents for meaningful file functionality:

```text
POST /v1/files
GET /v1/files
GET /v1/files/{id}
GET /v1/files/{id}/content
DELETE /v1/files/{id}
```

Files belong to the local artifact store.

---

# 32. BATCH API

Implement real local batch processing:

```text
validate
queue
run
persist output
report failures
```

Use asynchronous jobs.

---

# 33. ANTHROPIC COMPATIBILITY

Implement:

```text
POST /v1/messages
POST /v1/messages/count_tokens
```

and inspect the current Anthropic API surface during implementation.

Support, where locally meaningful:

```text
system
messages
content blocks
text
image
tools
tool_choice
streaming
max_tokens
temperature
top_p
top_k
stop_sequences
```

---

# 34. ANTHROPIC HEADER HANDLING

Handle current provider-compatible headers when relevant:

```text
x-api-key
anthropic-version
anthropic-beta
```

Do not require provider-specific headers that the local API does not need unless compatibility requires them.

---

# 35. PROVIDER NORMALIZATION

Do not implement separate agent engines.

Create:

```text
OpenAI → UnifiedRequest
Anthropic → UnifiedRequest
HCS → UnifiedRequest
```

Then:

```text UnifiedRequest
→ orchestration
→ UnifiedResponse
```

Then:

```text UnifiedResponse
→ OpenAI serializer
```

or:

```text UnifiedResponse
→ Anthropic serializer
```

---

# 36. UNIFIED CONTENT MODEL

Support internal content blocks for:

```text
text
image
tool_use
tool_result
```

Extend only when required by the current API compatibility surface.

---

# 37. STREAMING

Use one internal event stream:

```text
message_start
reasoning_delta
text_delta
tool_call_start
tool_call_delta
tool_result
artifact_created
message_end
error
```

Serialize into:

```text OpenAI SSE
```

or:

```text Anthropic SSE
```

---

# 38. TOOL CALL STREAMING

Never execute incomplete tool JSON.

Buffer tool call fragments until:

```text valid schema
valid JSON
policy accepted
```

then execute.

---

# 39. MODEL ROUTER

Support:

```text explicit model mode
automatic smart mode
```

Example:

```json
{
  "model": "auto"
}
```

or:

```json
{
  "model": "hcs-coder"
}
```

---

# 40. SMART ROUTING

Router inputs:

```text task category
modality
input size
context length
tools
vision
image generation
reasoning
latency target
memory availability
queue
model state
warm/cold status
explicit user model
fallback permission
```

---

# 41. SMART ROUTING RULE

Use:

```text smallest capable model
```

Examples:

```text simple parsing
→ hcs-subagent

normal reasoning
→ hcs-general

complex coding
→ hcs-coder

candidate decision
→ hcs-judge

image understanding
→ hcs-vlm

image generation/edit
→ hcs-image
```

---

# 42. ROUTING TRACE

Every automatic decision must record:

```text request_id
requested_model
selected_model
candidate_models
rejected_models
reason
memory state
queue state
fallback state
```

Display it in the dashboard.

---

# 43. MODEL MANAGER

State machine:

```text
COLD
VALIDATING
LOADING
WARMING
READY
ACTIVE
IDLE
SLEEPING
EVICTING
FAILED
QUARANTINED
DISABLED
```

---

# 44. HDD-FIRST MODEL LIFECYCLE

Models remain on HDD as durable source of truth.

Lifecycle:

```text HDD
→ load
→ RAM/Vulkan
→ active
→ idle
→ sleep/unload
→ cold
```

Do not copy models into arbitrary temporary locations without tracking the copy.

---

# 45. SSD HOT CACHE

If configured:

```text HDD
→ SSD cache
→ active memory
→ Vulkan
```

SSD cache is disposable.

HDD remains source of truth.

---

# 46. MODEL LOAD VALIDATION

Before load:

```text file exists
hash matches
manifest valid
backend compatible
memory estimated
dependencies present
```

Then:

```text load
warmup
health
ready
```

---

# 47. MEMORY MANAGER

Before every heavy load calculate:

```text model weights
KV cache
MTP
mmproj
compute buffers
image buffers
runtime overhead
system reserve
emergency reserve
```

Reject unsafe loads.

---

# 48. HEAVY MODEL POLICY

Default heavy group:

```text hcs-coder
hcs-vlm
hcs-image
```

Default:

```text max_heavy_active = 1
```

This may only be relaxed by evidence-backed benchmarking.

---

# 49. KV CACHE PROFILES

Support per-model profiles:

```text safe
balanced
performance
long-context
debug
```

Possible tested configurations:

```text F16/F16
Q8/Q8
Q4/Q4
Q4/F16
```

only where the exact backend supports them.

---

# 50. KV CORRECTNESS

Never adopt a KV mode because it saves memory alone.

Each candidate must pass:

```text deterministic output test
short context
medium context
long context where supported
streaming
tool use
```

---

# 51. FLASH ATTENTION

Support:

```text flash_attention = auto
```

and explicit:

```text on
off
```

where the backend exposes the feature.

Test every production configuration.

Do not transfer CUDA benchmark assumptions directly to AMD Vulkan.

---

# 52. MTP

MTP must be benchmark-driven.

Test:

```text off
1 draft token
2
4
```

where supported.

Measure:

```text acceptance
latency
throughput
memory
correctness
```

Only promote the configuration that passes all relevant gates.

---

# 53. SPECULATIVE DECODING

Keep support abstract:

```text MTP
future speculative methods
```

but do not put experimental algorithms into production without verification.

---

# 54. PROMPT CACHE

Support:

```text aggressive
safe
disabled
```

for prompt caching.

Use caching for repeated:

```text system prompt
tool definitions
repository instructions
stable context
```

but disable it automatically if reproducible corruption is detected.

---

# 55. CONTEXT MANAGEMENT

Do not automatically allocate the maximum supported context.

Profiles:

```text 4K
8K
16K
32K
64K
128K
262K
```

only where supported.

Choose the smallest sufficient profile.

---

# 56. CONTEXT COMPACTION

When context pressure becomes high:

```text extract durable facts
extract decisions
extract unresolved tasks
extract relevant files
extract current tool state
persist memory
compress context
continue
```

Do not throw away critical state silently.

---

# 57. AGENT RUNTIME

The agent is part of the same local daemon.

No external VM.

Components:

```text Supervisor
Planner
Researcher
Coder
Tester
Reviewer
Visual QA
Memory
Recovery
Tool Executor
```

---

# 58. AGENT TASK GRAPH

Use a DAG/task graph:

```text inspect
→ analyze
→ plan
→ implement
→ build
→ test
→ diagnose
→ repair
→ verify
→ review
→ complete
```

Each node stores:

```text state
model
tool calls
results
evidence
timestamps
artifacts
```

---

# 59. AGENT MODEL ASSIGNMENT

Default:

```text
small task
→ hcs-subagent

moderate planning
→ hcs-general

complex coding
→ hcs-coder

verification decision
→ hcs-judge

visual inspection
→ hcs-vlm
```

---

# 60. AGENT SYSTEM PROMPT

Create:

```text
prompts/agent-system.md
```

Use this as the core behavior contract:

```text
You are HCS Agent.

Your purpose is to complete the user's task correctly, reproducibly, safely and verifiably.

Reality has priority over narrative.

Never claim an action happened unless an observed tool result proves it.

Never fabricate command output, test results, files, model behavior, API responses, screenshots, benchmarks, memory, citations or system state.

Before modifying an unfamiliar project:
1. inspect the repository,
2. inspect all applicable project instructions,
3. inspect the architecture,
4. inspect configuration,
5. identify relevant files,
6. identify available tests,
7. create a minimal implementation plan.

Prefer small, targeted changes.

Do not modify unrelated code.

Use the smallest capable model for each task.

When a task requires deeper reasoning, escalate to the configured specialist model.

When using tools:
- execute real tools,
- observe actual results,
- never invent output,
- never claim success before verification.

When a command fails:
1. capture the exact error,
2. classify the failure,
3. localize the likely cause,
4. formulate a concrete hypothesis,
5. make the smallest justified change,
6. rerun the relevant test,
7. compare the result.

Do not use random edits as debugging.

For coding tasks:
- inspect Git state,
- preserve user work,
- use checkpoints or branches,
- inspect diffs,
- build,
- test,
- regression-test,
- review.

For visual tasks:
- inspect actual screenshots/images through the VLM,
- never claim to have visually verified something without an actual VLM observation.

For API tasks:
- preserve provider semantics,
- validate schemas,
- return truthful unsupported errors,
- do not fake compatibility.

For memory:
- distinguish temporary context from durable memory,
- store durable information only when evidence supports it,
- preserve provenance,
- support invalidation and correction.

For self-healing:
- recover safely,
- preserve evidence,
- never conceal failure,
- never enter infinite retry loops,
- detect repeated no-progress states,
- stop with an evidence-rich diagnostic when the task is genuinely blocked.

Completion requires proof.

A task is complete only when its required outputs exist and the relevant verification checks pass.
```

---

# 61. AGENT TOOL SYSTEM

Implement real tools for:

```text
filesystem
search
read
write
patch
git
shell
python
build
test
process
screenshot
memory
search/fetch
```

Use typed schemas.

---

# 62. TOOL PERMISSIONS

Every tool receives:

```text
risk
permissions
allowed paths
timeout
network permissions
```

---

# 63. COMMAND EXECUTION

Record:

```text
command
working directory
stdout
stderr
exit code
duration
request_id
agent_step_id
```

---

# 64. NO INFINITE REPAIR

Default:

```text
max_steps: 100
max_tool_calls: 200
max_retries_per_step: 3
max_repair_cycles: 5
max_same_failure: 3
```

When repeated failures show no progress:

```text
STOP
```

and produce a diagnostic.

---

# 65. SELF-HEALING LEVELS

Level 1:

```text transient retry
```

Level 2:

```text worker restart
```

Level 3:

```text model reload
```

Level 4:

```text agent diagnosis + repair
```

Every recovery is recorded.

---

# 66. WORKER WATCHDOG

Monitor:

```text hcs daemon
Prism worker
sd.cpp worker
```

Health:

```text healthy
starting
loading
busy
sleeping
failed
recovering
quarantined
```

---

# 67. CRASH RECOVERY

On worker crash:

```text capture logs
capture exit code
capture request ID
mark unhealthy
cleanup state
restart
reload
warmup
smoke test
retry at most once
```

Never retry forever.

---

# 68. MODEL QUARANTINE

If repeated failures exceed threshold:

```text QUARANTINED
```

The dashboard must show:

```text model
reason
last failure
restart count
last known working run
```

---

# 69. PERSISTENT MEMORY

Use SQLite initially.

Separate:

```text session state
agent run state
project memory
durable memory
historical archive
```

---

# 70. MEMORY TYPES

Support:

```text project_fact
decision
bug
fix
verified_result
research_fact
architecture_fact
tool_pattern
```

---

# 71. MEMORY ENTRY PROVENANCE

Every durable memory entry should record:

```text source
run_id
created_at
updated_at
last_verified
confidence
verified
hash
```

---

# 72. MEMORY SEARCH

Initial implementation may use:

```text SQLite FTS5
lexical ranking
metadata
path relevance
recency
exact search
```

Do not fabricate semantic embeddings.

---

# 73. OPTIONAL EMBEDDING EXTENSION

Architect a real embedding provider interface.

Do not add a fake embedding model.

If the project later contains an approved real embedding model:

```text embedding provider
→ vector index
→ reranking
```

can be enabled.

Until then:

```text lexical retrieval
```

is the honest default.

---

# 74. DASHBOARD

The browser dashboard must be a real WebAssembly application.

Preferred:

```text Rust
Leptos/Yew or another validated Rust/WASM stack
```

The final daemon serves it directly.

---

# 75. DASHBOARD MUST NEVER BE MOCKED

All displayed values come from:

```text real API
real telemetry
real database
real worker state
```

No hard-coded demo statistics.

---

# 76. DASHBOARD PAGES

Implement:

```text Overview
Requests
Models
Runtime
Routing
Memory
Agent
Jobs
Images
Files
API Explorer
API Keys
Logs
Benchmarks
Evaluations
System
Settings
Documentation
About
```

---

# 77. OVERVIEW

Show live:

```text daemon status
uptime
CPU
RAM
unified memory
Vulkan device
active model
loaded models
queue
requests
tokens
latency
errors
agent runs
image jobs
worker health
```

---

# 78. REQUESTS VIEW

Show:

```text timestamp
request ID
client IP
API provider
endpoint
API key ID
requested model
selected model
routing reason
status
input tokens
output tokens
total tokens
latency
```

Do not expose raw API secrets.

---

# 79. MODEL VIEW

Show:

```text model
source
file
size
hash
backend
state
capabilities
context
quantization
memory
last loaded
last used
load count
failure count
average latency
tokens/sec
```

---

# 80. ROUTING VIEW

Show:

```text request
→ candidate models
→ memory check
→ selected model
→ reason
```

---

# 81. TOKEN VIEW

Track:

```text input
output
total
tokens/sec
```

by:

```text request
model
API key
day
week
month
agent run
```

---

# 82. CLIENT/IP VIEW

Display controlled telemetry:

```text client IP
user agent
API key ID
first seen
last seen
request count
token usage
```

Respect configurable retention.

Do not log sensitive content by default.

---

# 83. LOG VIEW

Live categories:

```text Gateway
API
Router
Scheduler
Model Manager
Prism
Image
Agent
Tools
Memory
Security
```

---

# 84. AGENT VIEW

Show:

```text active runs
completed
failed
blocked
repair cycles
tool calls
models used
duration
```

Open a run to inspect:

```text goal
plan
steps
tools
test results
errors
repairs
screenshots
final verification
```

---

# 85. API KEY MANAGEMENT

Provide a real key system.

Support:

```text create
rename
rotate
revoke
expire
permission assignment
model restrictions
```

Only store secure hashes of API secrets.

Show the secret only once on creation.

---

# 86. API KEY PERMISSIONS

Support at least:

```text chat
responses
images
files
agent
memory
models
dashboard
admin
```

---

# 87. API EXPLORER

Browser UI must be able to send real:

```text OpenAI
Anthropic
HCS
```

requests.

Display:

```text status
headers
body
timing
usage
routing
request ID
```

---

# 88. SYSTEM PAGE

Show:

```text OS
CPU
RAM
GPU/iGPU
Vulkan
driver
disk
paths
ports
daemon
workers
versions
```

---

# 89. BENCHMARK PAGE

Run actual:

```text model benchmark
KV benchmark
Flash Attention benchmark
MTP benchmark
context benchmark
load/unload benchmark
image benchmark
VLM benchmark
API benchmark
agent benchmark
```

All displayed values must be real.

---

# 90. VISUAL QA OF DASHBOARD

Use a real browser.

Test:

```text desktop
laptop
tablet
mobile
dark mode
light mode
empty states
loading
errors
tables
graphs
dialogs
key creation
API Explorer
agent traces
```

Capture real screenshots.

Use the VLM for visual inspection.

---

# 91. UI ACCEPTANCE

A dashboard feature is complete only when:

```text frontend compiles
backend endpoint exists
real data reaches frontend
user action affects real backend
E2E test passes
visual inspection passes
```

---

# 92. API DATA FLOW

Everything must pass through a traceable pipeline:

```text client
→ authentication
→ normalization
→ routing
→ scheduling
→ model manager
→ backend
→ response normalization
→ provider serialization
→ telemetry
```

---

# 93. REQUEST TRACE

Every request receives:

```text request_id
```

Link:

```text request
job
agent run
agent step
model run
tool call
artifact
```

---

# 94. DATABASE

Use SQLite initially.

Suggested tables:

```text users
api_keys
requests
responses
model_runs
model_states
routing_decisions
agent_runs
agent_steps
tool_calls
tool_results
memory_items
files
artifacts
jobs
benchmarks
eval_runs
errors
audit_events
system_metrics
```

---

# 95. SECURITY

Default:

```text local-only
```

Default binding:

```text 127.0.0.1
```

LAN access must be explicitly enabled.

When enabled:

```text API key required
firewall guidance
allowed clients
rate limits
```

---

# 96. CORS

Default:

```text same-origin
```

Only configured origins may be permitted.

Do not use:

```text *
```

as a default.

---

# 97. REQUEST SIZE LIMITS

Implement configurable limits for:

```text request body
upload
image
file
agent output
tool output
batch
```

---

# 98. RATE LIMITS

Support per API key:

```text requests/min
tokens/min
concurrency
image jobs
agent jobs
```

---

# 99. SECRET REDACTION

Redact secrets from:

```text logs
dashboard
diagnostic bundles
agent context
benchmark reports
release artifacts
```

unless explicitly required and permitted.

---

# 100. AUDIT LOG

Audit security-sensitive actions:

```text API key creation
API key revocation
permission changes
model loading
model unloading
configuration changes
daemon restart
agent start
agent cancel
admin actions
```

---

# 101. ARTIFACT STORE

Use:

```text outputs/
├── images/
├── screenshots/
├── files/
├── agents/
└── diagnostics/
```

---

# 102. IMAGE ARTIFACT METADATA

Store:

```text model
prompt
seed
dimensions
steps
runtime
request_id
timestamp
hash
```

only where actually available.

Do not invent missing values.

---

# 103. DIAGNOSTIC BUNDLE

Implement:

```text hcs diagnostics
```

to create a sanitized archive containing:

```text hardware
runtime manifest
model manifests
health state
logs
config with secrets removed
benchmark summary
compatibility summary
```

---

# 104. CROSS-PLATFORM PATHS

Use platform-neutral filesystem APIs.

Do not hard-code drive letters in application logic.

The user's current path can be the default development path, but the product must support arbitrary installation paths.

---

# 105. CONFIGURATION

Implement:

```text config/default.yaml
config/development.yaml
config/production.yaml
config/models.yaml
config/security.yaml
config/compatibility.yaml
```

Environment overrides must work.

---

# 106. CONFIG VALIDATION

Startup must:

```text parse
validate
normalize
```

If invalid:

```text fail clearly
```

Do not silently ignore settings.

---

# 107. CLI

Implement at least:

```text hcs start
hcs stop
hcs restart
hcs status
hcs doctor
hcs test
hcs integration-test
hcs api-test
hcs stress
hcs agent-test
hcs ui-test
hcs benchmark
hcs models
hcs logs
hcs package
hcs release-verify
```

Do not document commands before they actually exist.

---

# 108. hcs doctor

Check:

```text OS
CPU
memory
Vulkan
driver
runtime binaries
DLLs/shared libraries
models
model hashes
configuration
database
ports
dashboard
```

Output exact evidence.

---

# 109. TEST PYRAMID

Implement:

```text unit
integration
contract
end-to-end
stress
recovery
visual
release
```

---

# 110. OPENAI CONTRACT TESTS

Use real requests.

Test:

```text models
chat completions
responses
files
batches
images
embeddings behavior
moderations behavior
```

where supported.

---

# 111. ANTHROPIC CONTRACT TESTS

Use real requests for:

```text messages
count_tokens
streaming
tools
tool results
images
errors
authentication
```

---

# 112. REAL SDK TESTS

Where current compatible SDK versions are available, test:

```text OpenAI Python SDK
OpenAI JavaScript/TypeScript SDK
Anthropic Python SDK
Anthropic TypeScript SDK
curl
```

The exact tested SDK versions must be documented.

---

# 113. MODEL SMOKE TESTS

Each real model:

```text validate
load
warm
real inference
stream
health
unload
reload
```

---

# 114. OPENJEV TESTS

Run real OpenJev contract tests:

```text 2 candidates
multiple candidates
maximum candidate count supported by contract
A–P mapping
strict output
invalid candidate
missing candidate
malformed model output
```

Verify:

```text candidate ID
decision
provenance
```

---

# 115. IMAGE TESTS

Run:

```text T2I
I2I
editing
reference editing if supported
```

Save actual generated outputs.

Verify:

```text file
dimensions
format
hash
metadata
```

---

# 116. VLM TESTS

Use actual images:

```text UI
document
screenshot
chart
small text
complex image
```

Record:

```text model
mmproj
latency
output
```

---

# 117. AGENT TEST

Create a temporary real repository containing an intentional defect.

Ask the agent to:

```text inspect
plan
modify
build
test
observe failure
diagnose
repair
test again
review
finish
```

The dashboard must show the entire process.

---

# 118. VISUAL AGENT TEST

Create or use a small real UI project.

Agent:

```text modify UI
build
launch
screenshot
VLM analysis
repair
rebuild
screenshot
verify
```

No fake screenshots.

---

# 119. IMAGE AGENT TEST

Agent:

```text create image
store image
inspect image using VLM
judge result
edit/retry if configured
return actual artifact
```

---

# 120. MEMORY PERSISTENCE TEST

Test:

```text store verified memory
restart daemon
retrieve
verify provenance
```

Memory must survive restart.

---

# 121. CRASH RECOVERY TEST

During real inference:

```text terminate worker
```

Verify:

```text watchdog detects
logs capture
worker restarts
model reloads
smoke test
next request succeeds
```

---

# 122. MEMORY PRESSURE TEST

Create controlled pressure.

Verify:

```text scheduler refuses unsafe load
idle models evict
jobs remain queued
daemon remains alive
```

---

# 123. MODEL SWITCHING STRESS

Cycle repeatedly:

```text general
coder
VLM
image
general
coder
image
VLM
```

Track:

```text crashes
memory leaks
load failures
stale processes
latency
```

---

# 124. COLD/WARM STRESS

Repeatedly:

```text cold load
request
unload
load
request
```

dozens of times.

---

# 125. API STRESS

Run controlled:

```text sequential requests
small concurrency
mixed model queue
streaming
cancellation
timeouts
```

Do not overwhelm the machine purely to obtain an artificial benchmark number.

---

# 126. TOOL STRESS

Test chains:

```text read
search
write
build
test
read error
patch
test
review
```

---

# 127. AGENT FAILURE STRESS

Inject:

```text syntax error
compile error
failing test
missing file
wrong path
dependency issue
backend timeout
worker crash
VLM rejection
```

The agent must recover correctly or stop honestly with evidence.

---

# 128. NO-PROGRESS TEST

Force the agent into a repeated failure.

Verify it eventually reaches:

```text BLOCKED
```

rather than an infinite loop.

---

# 129. SECURITY TESTS

Test:

```text path traversal
command injection
auth bypass
permission bypass
invalid API key
expired key
revoked key
oversized request
malicious upload
secret leakage
log injection
```

---

# 130. RELEASE BUILD STRUCTURE

Create release artifacts only from clean builds.

Windows:

```text
HCS-Local-AI-Setup-x64.exe
HCS-Local-AI-Portable-x64.zip
HCS-Local-AI-Windows-x64.zip
```

Linux, only if genuinely built:

```text
HCS-Local-AI-Linux-x86_64.tar.gz
HCS-Local-AI-Linux-x86_64.AppImage
HCS-Local-AI-Linux-amd64.deb
```

Do not invent artifacts for platforms that did not build successfully.

---

# 131. WINDOWS INSTALLER

Installer must:

```text install daemon
install CLI
install dashboard
install runtime binaries
create data directories
optionally register service
create uninstaller
```

It must preserve existing model files.

---

# 132. LINUX INSTALLER

Support native Linux deployment with:

```text systemd
```

and at least one practical package/distribution format when the build environment supports it.

---

# 133. PORTABLE MODE

Produce a portable package where possible:

```text hcs.exe
runtime/
dashboard/
config/
models/ references
```

Portable mode must actually work.

---

# 134. FIRST RUN

The first-run system should:

```text detect hardware
detect Vulkan
discover models
validate binaries
validate hashes
initialize database
create configuration
create initial admin/API credentials
```

---

# 135. API KEY FIRST-RUN

Display the initial secret only once.

Persist only a secure hash.

If the user loses the secret:

```text rotate/recreate
```

instead of recovering plaintext.

---

# 136. DASHBOARD FIRST RUN

Show:

```text system status
Vulkan device
detected models
missing/invalid models
API endpoint
Anthropic endpoint
dashboard URL
key setup
```

---

# 137. OFFLINE MODE

After dependencies are installed, test local operation without internet.

The local stack must continue to support:

```text local API
local models
dashboard
agent
memory
image
VLM
```

without silently requiring a cloud model provider.

---

# 138. WEB RESEARCH DURING DEVELOPMENT

Use current official documentation when implementing:

```text OpenAI API
Anthropic API
llama.cpp
Prism
stable-diffusion.cpp
Rust dependencies
WASM framework
browser automation
packaging tools
GitHub Actions
```

Do not depend on stale blog posts for current API semantics.

Record verification dates for externally changing compatibility requirements.

---

# 139. REPOSITORY INVENTORY FIRST

Before creating or modifying substantial source code:

```text recursively inspect current directory
```

Identify:

```text files
models
binaries
source
scripts
configs
documentation
tests
existing Git
existing remote
```

Produce:

```text docs/initial-inventory.md
```

---

# 140. PRESERVE EXISTING WORK

Before destructive restructuring:

```text inspect
backup/checkpoint
Git status
```

Never delete existing assets simply because a clean project structure is desired.

Move/rename only when justified.

---

# 141. DETERMINE WHAT IS MISSING

Create:

```text docs/missing-components.md
```

Classify:

```text present and valid
present but invalid
present but outdated
missing and required
missing but optional
not needed
```

---

# 142. ACQUIRE MISSING COMPONENTS

After inspection:

```text fetch only what is genuinely missing
```

Potentially missing components include:

```text model companion files
runtime DLLs
runtime shared libraries
Prism runtime
sd.cpp runtime
dashboard dependencies
Rust crates
installer tooling
browser test tooling
documentation references
OpenJev contract files
```

Do not download random models.

---

# 143. DOWNLOAD VALIDATION

Every downloaded artifact:

```text checksum
size
source URL/repository
revision
license
```

then move into its canonical location.

Never treat a partially downloaded file as valid.

---

# 144. MODEL DOWNLOAD POLICY

Use deterministic downloads.

Prefer pinned revisions.

Do not use uncontrolled:

```text latest
main
master
```

for release-critical components unless the exact state is captured and hashed afterward.

---

# 145. GIT REPOSITORY CREATION

If the current directory is not already the desired repository:

```text initialize repository
```

with:

```text clean structure
.gitignore
.gitattributes
.editorconfig
LICENSE
README
docs
tests
CI
packaging
```

---

# 146. GITHUB REPOSITORY

If GitHub CLI authentication is available:

```text create a new repository
set origin
```

Use the intended repository name.

Do not push until the stable release gate is green.

---

# 147. GIT HISTORY

Use meaningful commits:

```text feat:
fix:
refactor:
perf:
test:
docs:
build:
ci:
security:
release:
```

Do not produce an opaque giant development history if meaningful incremental commits are practical.

---

# 148. GITIGNORES

Ignore:

```text secrets
.env
cache
temporary data
local DB where appropriate
build artifacts
test browser artifacts
logs
```

Do not ignore source/tests/docs accidentally.

---

# 149. README DESIGN

The README must be polished and honest.

Required structure:

```text
Hero
Overview
Architecture
Features
Supported Models
Requirements
Quick Start
Windows
Linux
OpenAI API
Anthropic API
Image API
Agent
Dashboard
API Keys
Memory
Model Management
Configuration
Performance
Benchmarks
Security
Troubleshooting
Known Limitations
Compatibility Matrix
Build From Source
Packaging
Release Verification
License
Credits
```

---

# 150. README HERO

Use a professional HCS visual identity.

Provide locally stored:

```text docs/assets/logo.svg
docs/assets/logo-dark.svg
docs/assets/logo-mark.svg
docs/assets/banner.svg
```

Do not use copyrighted third-party logos as the main brand.

Provider references may use text rather than copied brand artwork when appropriate.

---

# 151. LOGO DESIGN

Create a consistent local HCS identity:

```text HCS
Local AI
Vulkan
Agentic
```

Use SVG so the dashboard and README can scale without raster blur.

---

# 152. README BADGES

Use real badges only:

```text CI
Windows build
Linux build
release
license
tests
```

Do not create fake "100% working" badges.

---

# 153. README STATUS

Include a real status matrix:

```text Component | Status | Evidence
```

Example fields:

```text OpenAI Chat
OpenAI Responses
Anthropic Messages
Image Generation
Image Editing
VLM
Agent
Dashboard
Windows Package
Linux Package
Vulkan Runtime
Stress Suite
```

---

# 154. HONEST LIMITATIONS

Document everything that is:

```text experimental
unsupported
hardware-dependent
license-restricted
platform-specific
not available in cloud CI
```

Do not hide limitations.

---

# 155. AUTOMATED DOCUMENTATION GENERATION

Generate current API documentation from the actual backend schema.

Avoid manually maintained fictional endpoint lists.

---

# 156. COMPATIBILITY MATRIX

Create:

```text docs/compatibility-matrix.md
```

Include:

```text endpoint
provider
implemented
streaming
tools
vision
image
tested SDK
test ID
last verified
limitations
```

---

# 157. SHA256 VERIFICATION

Create:

```text SHA256SUMS.txt
```

for release artifacts.

Also maintain per-artifact manifest data.

---

# 158. MODEL HASH VERIFICATION

Create:

```text docs/model-verification.md
```

with:

```text model
source
revision
size
sha256
backend
verification date
verification run ID
```

---

# 159. RUN MANIFESTS

Each major test run must produce a JSON manifest such as:

```text
runs/
├── local-smoke/
├── local-api/
├── local-agent/
├── local-image/
├── local-vlm/
├── local-stress/
├── local-recovery/
├── cloud-build/
└── release/
```

Each manifest records:

```text run_id
timestamp
Git commit
model hashes
runtime hashes
config hash
hardware
tests
results
artifacts
```

---

# 160. VERIFICATION HASH CHAIN

Where practical, create a root verification manifest containing hashes of:

```text source revision
runtime binaries
model files
config
dashboard build
installer artifacts
test report
```

This makes the final release reproducible/auditable.

---

# 161. LOCAL GOLDEN RUN

After the system works:

```text create a complete golden run
```

including:

```text model smoke
API
Anthropic
image
VLM
agent
dashboard
memory
stress
recovery
```

Freeze the environment metadata.

---

# 162. GOLDEN CONFIGURATION

Store:

```text config/golden/
```

with per-model verified settings:

```text context
KV
Flash Attention
MTP
batch
ubatch
parallelism
offload
timeouts
```

---

# 163. AUTOTUNING

Build an experimental autotuner.

Test:

```text context
KV
Flash Attention
MTP
batch
ubatch
parallel
offload
```

Optimization priority:

```text correctness
→ stability
→ memory
→ performance
```

---

# 164. RELEASE PROFILE

Production uses:

```text known-good runtime
known-good model
known-good configuration
known-good API
```

Experimental tuning must not silently alter production.

---

# 165. CLEAN BUILD

At least once before release:

```text remove build outputs
build dependencies cleanly
compile from clean state
```

Ensure no hidden developer-machine artifact is required.

---

# 166. OFFLINE CLEAN BUILD WHERE PRACTICAL

After dependency acquisition, verify the project can build from cached/local dependencies without silently depending on an unavailable network resource where practical.

Document unavoidable external requirements.

---

# 167. CI/CD

Create GitHub Actions workflows:

```text .github/workflows/
├── ci.yml
├── compatibility.yml
├── windows-build.yml
├── linux-build.yml
├── stress.yml
├── security.yml
└── release.yml
```

---

# 168. CI BASIC PIPELINE

Every push/PR:

```text formatting
static analysis
unit tests
compile
API schema validation
frontend compile
```

---

# 169. WINDOWS CI

Build:

```text native backend
dashboard WASM
Windows package
portable artifact
installer
```

and run all tests possible in the GitHub-hosted environment.

---

# 170. LINUX CI

Build:

```text native Linux backend
dashboard
Linux package
```

where the runner supports required dependencies.

---

# 171. VULKAN CI TRUTH RULE

A successful compilation does NOT prove runtime Vulkan inference.

Do not label:

```text compiled Vulkan
```

as:

```text runtime Vulkan verified
```

unless a matching Vulkan-capable runner actually executes it.

---

# 172. VULKAN RUNTIME VERIFICATION

For real Vulkan runtime verification use:

```text suitable self-hosted runner
or
local verified hardware
```

when cloud-hosted workers do not provide the correct AMD/Vulkan environment.

Document exactly where runtime verification happened.

---

# 173. CLOUD ARTIFACT VERIFICATION

After GitHub Actions finishes:

```text download artifacts
verify hashes
inspect package contents
install/run where possible
run smoke tests
```

Do not trust CI merely because the job is green.

---

# 174. LOCAL REVERIFICATION AFTER CLOUD BUILD

This is mandatory.

After cloud artifacts are produced:

```text use the actual generated artifact
install/run locally
repeat critical smoke tests
```

At minimum:

```text daemon
dashboard
OpenAI
Anthropic
model loading
image
VLM
agent
memory
```

---

# 175. RELEASE GATE

Do not create `v1.0.0` until:

```text clean build PASS
unit tests PASS
integration tests PASS
OpenAI contract tests PASS
Anthropic contract tests PASS
model smoke PASS
OpenJev contract PASS
image T2I PASS
image I2I PASS
VLM PASS
agent PASS
memory persistence PASS
dashboard E2E PASS
visual QA PASS
stress PASS
recovery PASS
security tests PASS
Windows package PASS
Linux package PASS where claimed
documentation PASS
hash verification PASS
```

A genuinely platform-unavailable test is:

```text NOT_AVAILABLE
```

not:

```text PASS
```

---

# 176. TEST STATUS SEMANTICS

Allowed statuses:

```text PASS
FAIL
SKIPPED
NOT_AVAILABLE
EXPERIMENTAL
BLOCKED
```

Never convert:

```text SKIPPED
```

into:

```text PASS
```

---

# 177. FINAL RELEASE REPORT

Generate:

```text docs/release-verification.md
```

containing:

```text release
Git commit
hardware
OS
runtime versions
model versions
hashes
test commands
test IDs
results
stress results
recovery results
dashboard results
package results
known limitations
```

---

# 178. RELEASE MANIFEST

Create:

```text release-manifest.json
```

with:

```text version
Git commit
build timestamp
platform
architecture
runtime hashes
model hashes
dashboard hash
configuration hash
test report hash
artifact hashes
verification status
```

---

# 179. RELEASE NOTES

Generate:

```text CHANGELOG.md
docs/release-notes-v1.0.0.md
```

based on actual changes.

Include:

```text added
changed
fixed
verified
experimental
known limitations
```

No invented claims.

---

# 180. SECURITY REVIEW

Before release:

```text scan dependencies
inspect secrets
inspect Git history
inspect package contents
inspect installer permissions
inspect upload limits
inspect command execution
inspect path validation
inspect authentication
```

---

# 181. SECRET SCAN

Search repository and artifacts for:

```text API keys
tokens
private keys
passwords
credential files
machine-specific secrets
```

Do not push if secrets are found.

---

# 182. LICENSE REVIEW

Review:

```text project dependencies
runtime licenses
model licenses
dashboard dependencies
fonts/assets if any
```

Record attribution.

---

# 183. ASSET REVIEW

All logo/graphic assets must be:

```text original
generated
or properly licensed
```

Do not copy a provider's trademarked artwork into the HCS brand package.

---

# 184. INSTALLER TEST

Install the actual Windows installer in a clean test location/profile.

Verify:

```text installation
service
daemon
dashboard
OpenAI
Anthropic
models
image
VLM
agent
memory
logs
uninstaller
```

---

# 185. PORTABLE TEST

Extract the actual portable archive.

Run without development tooling.

Verify:

```text daemon
dashboard
API
models
```

subject to model packaging/licensing rules.

---

# 186. LINUX TEST

Where the required Vulkan environment exists:

```text install package
start service
open dashboard
run OpenAI
run Anthropic
run model
run image
```

If a GitHub runner can only compile Linux Vulkan code but cannot execute the required GPU path:

```text document compile verification separately
```

---

# 187. API EXAMPLE TESTS

Every README example must be executable.

Create:

```text examples/
├── openai_chat.*
├── openai_responses.*
├── anthropic_messages.*
├── image_generation.*
├── image_edit.*
└── agent.*
```

Run them during release verification.

---

# 188. DOCUMENTATION DRIFT TEST

Automatically detect:

```text documented command missing
documented endpoint missing
documented file missing
documented model missing
```

---

# 189. DASHBOARD DOCUMENTATION

Document every major dashboard page:

```text purpose
data source
actions
permissions
limitations
```

---

# 190. MODEL DOCUMENTATION

For every model:

```text source
role
file
size
hash
backend
capabilities
context
verified configuration
known limitations
```

---

# 191. RUNTIME DOCUMENTATION

Document:

```text Prism
Vulkan
stable-diffusion.cpp
worker lifecycle
KV
MTP
Flash Attention
offloading
model loading
```

---

# 192. AGENT DOCUMENTATION

Document:

```text system prompt
roles
tools
permissions
task graph
memory
self-healing
recovery
verification
```

---

# 193. API DOCUMENTATION

Document:

```text OpenAI
Anthropic
HCS Native
```

using generated OpenAPI plus human-facing guides.

---

# 194. NO DEAD API ROUTES

Every route must either:

```text execute real functionality
```

or:

```text return an honest capability error
```

and have a test.

---

# 195. NO DEAD UI BUTTONS

Every visible dashboard action must:

```text call real API
```

or not exist.

Do not create decorative controls that do nothing.

---

# 196. NO DEAD CLI COMMANDS

Every documented CLI command must execute a real code path.

---

# 197. NO DEAD CONFIGURATION

Every configuration key must be:

```text parsed
validated
used
```

or removed from documentation.

---

# 198. API ERROR MODEL

Use stable internal errors such as:

```text MODEL_NOT_FOUND
MODEL_LOAD_FAILED
MODEL_UNSUPPORTED
BACKEND_UNAVAILABLE
BACKEND_CRASHED
MEMORY_UNAVAILABLE
QUEUE_FULL
AUTH_INVALID
AUTH_EXPIRED
PERMISSION_DENIED
INVALID_REQUEST
UNSUPPORTED_FEATURE
TOOL_FAILED
AGENT_TIMEOUT
AGENT_NO_PROGRESS
FILE_TOO_LARGE
ARTIFACT_NOT_FOUND
```

Serialize correctly for each provider.

---

# 199. TELEMETRY PRIVACY

Do not store complete prompts/responses by default.

Default telemetry:

```text metadata
timings
counts
IDs
state
```

Optional debug content logging must:

```text require explicit configuration
be clearly marked sensitive
respect retention
redact secrets
```

---

# 200. PERFORMANCE METRICS

Measure:

```text model load time
queue time
prefill
decode
latency
memory peak
KV memory
MTP acceptance
worker restarts
agent duration
tool duration
image generation duration
VLM duration
```

---

# 201. PERFORMANCE REPORT

Create:

```text docs/benchmarks.md
```

with real results.

Never compare different hardware as though it were the same machine.

Always document:

```text hardware
driver
runtime
model hash
configuration
```

---

# 202. GOLDEN REQUEST SET

Create stable request fixtures for:

```text fast/subagent
general
coder
OpenJev
VLM
image
agent
API compatibility
```

Use them for regression tests.

---

# 203. DETERMINISTIC TESTING

Where practical:

```text temperature = 0
fixed seed
fixed input
fixed model
fixed runtime
```

for regression tests.

Do not require byte-identical output when the backend cannot guarantee it; instead define semantic/structural acceptance criteria.

---

# 204. IMAGE REGRESSION

For image generation:

```text fixed seed
fixed prompt
fixed model
fixed runtime
```

Then verify:

```text output exists
dimensions
format
basic content constraints
```

Do not require pixel-identical output unless the runtime guarantees deterministic output.

---

# 205. VLM REGRESSION

Use structured expectations where possible.

Example:

```text screenshot contains button
VLM should identify button
```

Avoid relying solely on free-form text comparison.

---

# 206. AGENT REGRESSION

The agent is successful if:

```text requested artifact exists
tests pass
diff is reasonable
required actions occurred
```

not because the final prose sounds convincing.

---

# 207. OPENAI RESPONSE STRUCTURE

Return true provider-compatible structures.

Never leak internal HCS metadata into provider fields unless the provider allows extensions.

HCS metadata may be exposed through a controlled optional extension.

---

# 208. HCS EXTENSIONS

Support:

```json
{
  "extra_body": {
    "hcs": {
      "smart": true,
      "profile": "balanced",
      "offload": "auto",
      "kv": "auto",
      "mtp": "auto"
    }
  }
}
```

Validate every option.

---

# 209. MODEL CONTROL EXTENSIONS

HCS-native endpoints:

```text
POST /hcs/v1/models/{id}/load
POST /hcs/v1/models/{id}/warm
POST /hcs/v1/models/{id}/sleep
POST /hcs/v1/models/{id}/unload
POST /hcs/v1/models/{id}/restart
POST /hcs/v1/models/{id}/validate
POST /hcs/v1/models/{id}/benchmark
```

---

# 210. AGENT API

Implement:

```text
POST /hcs/v1/agent/runs
GET /hcs/v1/agent/runs
GET /hcs/v1/agent/runs/{id}
POST /hcs/v1/agent/runs/{id}/cancel
POST /hcs/v1/agent/runs/{id}/resume
GET /hcs/v1/agent/runs/{id}/events
```

---

# 211. MEMORY API

Implement:

```text
GET /hcs/v1/memory
POST /hcs/v1/memory
GET /hcs/v1/memory/{id}
PATCH /hcs/v1/memory/{id}
DELETE /hcs/v1/memory/{id}
POST /hcs/v1/memory/search
```

---

# 212. JOB API

Implement:

```text
GET /hcs/v1/jobs
GET /hcs/v1/jobs/{id}
POST /hcs/v1/jobs/{id}/cancel
```

---

# 213. EVENTS API

Implement:

```text
GET /hcs/v1/events
```

via SSE.

Optionally:

```text
GET /hcs/v1/ws
```

for WebSocket clients.

---

# 214. LIVE EVENTS

Use actual events:

```text request.created
request.queued
model.loading
model.ready
model.sleeping
model.evicted
backend.crashed
backend.recovered
agent.step
tool.started
tool.finished
benchmark.started
benchmark.finished
job.completed
```

---

# 215. AGENT VISUALIZATION

Dashboard must show:

```text
plan
current step
tool call
tool result
test
failure
repair
verification
```

as actual event data.

---

# 216. MODEL VISUALIZATION

Show:

```text cold
loading
ready
active
sleeping
evicted
failed
```

live.

---

# 217. MEMORY VISUALIZATION

Show:

```text total
available
AI budget
model memory
KV
runtime buffers
cache
system reserve
```

from actual telemetry.

---

# 218. TOKEN VISUALIZATION

Show:

```text input
output
total
tokens/sec
```

from actual request telemetry.

---

# 219. IP VISUALIZATION

Use actual client connection information.

Do not claim the IP came from an external lookup.

---

# 220. API KEY VISUALIZATION

Display:

```text key ID
name
permissions
created
last used
expires
status
```

Never display the secret after creation.

---

# 221. TEST REPORT UI

Dashboard should allow inspection of:

```text test suites
latest runs
PASS/FAIL
duration
hardware
runtime
model
artifact
```

---

# 222. RELEASE VERIFICATION UI

Display:

```text current Git commit
build
model hashes
runtime hashes
test summary
package status
```

---

# 223. STARTUP RESILIENCE

If an optional model is broken:

```text daemon may enter degraded mode
```

rather than refusing to start everything.

Show degraded state clearly.

---

# 224. CRITICAL COMPONENT FAILURE

If:

```text database
gateway
configuration
```

cannot initialize:

```text daemon fails clearly
```

Do not start in an apparently healthy but corrupted state.

---

# 225. MODEL LOAD FAILURE

Return:

```text model_load_failed
```

with:

```text request ID
model
stage
reason
```

---

# 226. IMAGE FAILURE

Return an explicit image backend error.

Never fall back to a text model for image generation.

---

# 227. VLM FAILURE

Never fall back to text-only reasoning while pretending visual analysis succeeded.

---

# 228. OPENJEV FAILURE

If OpenJev is unavailable:

```text hcs-judge unavailable
```

Do not fabricate decisions.

---

# 229. AGENT FAILURE

If agent becomes blocked:

```text blocked state
```

with evidence.

---

# 230. RELEASE ENGINE

Implement scripts:

```text scripts/
├── build.ps1
├── build.sh
├── start.ps1
├── stop.ps1
├── doctor.ps1
├── test.ps1
├── api-test.ps1
├── stress.ps1
├── ui-test.ps1
├── benchmark.ps1
├── package-windows.ps1
├── package-linux.sh
└── release-verify.ps1
```

Use equivalent platform-appropriate scripts where necessary.

---

# 231. RELEASE AUTOMATION

One release verification command should execute:

```text clean build
tests
package
hash
verify
```

and fail on the first critical gate.

---

# 232. RELEASE WORKFLOW

The release pipeline:

```text local clean build
→ local tests
→ local stress
→ local UI
→ local packaging
→ cloud CI
→ cloud package verification
→ download artifact
→ local reverify
→ release notes
→ hashes
→ tag
→ push
→ GitHub Release
```

---

# 233. GITHUB TAG

Only after all gates:

```text v1.0.0
```

---

# 234. GITHUB RELEASE

Upload:

```text Windows installer
Windows portable
Windows archive
Linux archive
Linux package(s)
SHA256SUMS
release-manifest.json
release notes
compatibility matrix
```

only for artifacts that genuinely exist and passed their corresponding tests.

---

# 235. RELEASE DRAFT

Before final publish:

```text create draft release
```

inspect all assets and notes.

Only then publish stable.

---

# 236. FINAL GIT STATE

Before pushing:

```text git status
git diff
```

Check:

```text no secrets
no local credentials
no irrelevant files
no temporary logs
no machine-private files
no accidental binaries
```

---

# 237. GIT PUSH

Push only after:

```text local final verification PASS
cloud verification PASS
local re-verification PASS
```

---

# 238. POST-PUSH VERIFICATION

After push:

```text verify GitHub repository
verify tag
verify release
verify release assets
verify checksums
verify README
verify workflows
```

Do not stop immediately after `git push`.

---

# 239. POST-RELEASE LOCAL CHECK

Use the exact release tag or downloaded release artifact.

Run:

```text install
start
health
OpenAI
Anthropic
image
VLM
agent
dashboard
```

Record final run.

---

# 240. RELEASE ANNOUNCEMENT DATA

Do not write hype claims.

Use actual facts:

```text version
platforms
verified APIs
verified models
verified runtimes
known limitations
```

---

# 241. AUTONOMOUS REPAIR LOOP

During development, whenever something fails:

```text observe
→ capture
→ classify
→ hypothesize
→ patch
→ build
→ retest
```

If the same failure repeats:

```text stop repeating identical action
→ reconsider architecture
→ inspect dependencies
→ inspect assumptions
→ choose a different justified strategy
```

---

# 242. SELF-HEALING OF THE BUILD AGENT

As the engineering agent:

```text if build fails:
inspect error

if dependency fails:
inspect dependency/version

if API test fails:
inspect request/response schema

if model fails:
inspect model metadata/runtime compatibility

if UI fails:
inspect browser console/network

if Vulkan fails:
inspect device/backend/runtime/driver

if installer fails:
inspect package layout/install logs
```

Never blindly regenerate the same broken code.

---

# 243. ROOT CAUSE ANALYSIS

For difficult failures create:

```text diagnostics/<failure-id>.md
```

containing:

```text observed failure
environment
evidence
likely cause
attempts
result
final fix
regression test
```

---

# 244. NO SILENT PATCHES

Every important repair should result in:

```text code diff
test
verification entry
```

---

# 245. VISUAL CODE VERIFICATION

For frontends:

```text compile
→ launch
→ browser
→ interact
→ screenshot
→ VLM
→ repair
→ retest
```

---

# 246. BACKEND VISUAL VERIFICATION

Dashboard values such as:

```text model state
memory
tokens
routing
request status
```

must be compared to backend reality during E2E tests.

---

# 247. UI/BACKEND CONSISTENCY TEST

Example:

```text dashboard says coder ACTIVE
```

must correspond to:

```text actual worker/model state
```

not a cached fictional value.

---

# 248. API/DASHBOARD CONSISTENCY

When an API request is sent:

```text request appears in dashboard
```

with matching:

```text request ID
model
status
token count
timing
```

---

# 249. AGENT/DASHBOARD CONSISTENCY

When the agent calls:

```text run_tests
```

the dashboard must show:

```text tool.started
tool.finished
actual stdout/stderr
exit code
```

---

# 250. RELEASE REPRODUCIBILITY

Record:

```text source commit
runtime commit
runtime hashes
model hashes
toolchain versions
OS
compiler
dashboard build
```

in the release manifest.

---

# 251. BUILD ENVIRONMENT RECORD

For each build:

```text Rust version
Cargo version
C/C++ compiler
CMake
Node/tooling if used
WASM toolchain
Vulkan SDK/runtime
```

where applicable.

---

# 252. DEPENDENCY PINNING

Pin important release dependencies.

Do not allow release builds to silently resolve arbitrary future versions.

---

# 253. UPDATE POLICY

A future runtime/model update must first enter:

```text experimental
```

then:

```text candidate
```

then:

```text verified
```

then:

```text production
```

---

# 254. EXPERIMENTAL FEATURES

Examples:

```text new MTP
new KV compression
new speculative decoder
new Vulkan kernel
new scheduler algorithm
```

must remain clearly marked.

---

# 255. PRODUCTION DEFAULT

Production must use the best **verified** configuration, not the most aggressive configuration.

---

# 256. FINAL MODEL ROUTING

Default:

```text hcs-subagent
→ tiny quick work

hcs-general
→ normal work

hcs-coder
→ complex coding

hcs-judge
→ decision/validation

hcs-vlm
→ visual understanding

hcs-image
→ image generation/editing
```

---

# 257. FINAL IMAGE PIPELINE

Expected structure:

```text user prompt/input
        ↓
HCS Image API
        ↓
artifact validation
        ↓
Qwen3-4B text encoder
        ↓
Bonsai FLUX.2 Klein ternary model
        ↓
stable-diffusion.cpp Vulkan
        ↓
FLUX.2 decoder/encoder path
        ↓
PNG/image artifact
        ↓
optional VLM verification
        ↓
final response
```

---

# 258. FINAL OPENJEV PIPELINE

```text agent/router
        ↓
candidate generation
        ↓
OpenJev contract
        ↓
hcs-judge
        ↓
A–P decision
        ↓
candidate ID
        ↓
real action
        ↓
observation
```

---

# 259. FINAL CODING AGENT PIPELINE

```text user
 ↓
supervisor
 ↓
planner
 ↓
relevant context
 ↓
hcs-coder
 ↓
tool
 ↓
real result
 ↓
test
 ↓
failure if any
 ↓
diagnosis
 ↓
repair
 ↓
test
 ↓
judge
 ↓
review
 ↓
completion
```

---

# 260. FINAL VISUAL CODING PIPELINE

```text code
 ↓
build
 ↓
launch
 ↓
screenshot
 ↓
hcs-vlm
 ↓
findings
 ↓
coder
 ↓
fix
 ↓
build
 ↓
screenshot
 ↓
VLM
 ↓
verified
```

---

# 261. FINAL DATA GENERATION PIPELINE

Optional after stable inference:

```text agent generates candidate
 ↓
tool execution
 ↓
tests
 ↓
judge
 ↓
visual verification when needed
 ↓
provenance
 ↓
deduplication
 ↓
verified dataset
```

Never feed unchecked synthetic data directly into training.

---

# 262. TRAINING

Training is an optional future stage.

Use only a genuinely supported training backend.

For the constrained local hardware, prioritize:

```text LoRA
QLoRA
small-model adaptation
MTP experiments
```

Do not pretend full large-model retraining is a supported feature if it is not.

---

# 263. TRAINING DATA PROVENANCE

Every synthetic example must contain:

```text sample ID
generator
critic
verification
source
timestamp
hash
dataset version
```

---

# 264. TRAINING RELEASE RULE

A trained model does not replace production automatically.

Process:

```text candidate
→ evaluate
→ regression
→ compare
→ promote
```

---

# 265. README BUILD STATUS

At release, README must distinguish:

```text locally runtime verified
cloud compiled
cloud runtime verified
not available
experimental
```

Do not collapse all of these into one green badge.

---

# 266. PLATFORM CLAIMS

Only claim:

```text Windows Vulkan runtime verified
```

if a real Windows Vulkan test succeeded.

Only claim:

```text Linux Vulkan runtime verified
```

if a real Linux Vulkan test succeeded.

Compilation alone is insufficient.

---

# 267. CLOUD LIMITATIONS

If the hosted CI does not expose the required Vulkan hardware:

```text say so explicitly
```

and use an appropriate real Vulkan-capable verification machine/runner where available.

Never fabricate cloud GPU execution.

---

# 268. FINAL RELEASE CHECKLIST

Before `v1.0.0`:

```text [ ] repository inspected
[ ] missing components resolved
[ ] licenses checked
[ ] models validated
[ ] hashes recorded
[ ] Prism validated
[ ] sd.cpp validated
[ ] daemon builds
[ ] dashboard builds
[ ] OpenAI API tested
[ ] Anthropic API tested
[ ] OpenJev tested
[ ] image T2I tested
[ ] image I2I tested
[ ] VLM tested
[ ] agent tested
[ ] memory persistence tested
[ ] tool execution tested
[ ] self-healing tested
[ ] crash recovery tested
[ ] memory pressure tested
[ ] stress tested
[ ] dashboard E2E tested
[ ] visual QA tested
[ ] security tests passed
[ ] clean build passed
[ ] Windows package passed
[ ] Linux package passed where claimed
[ ] SHA256 generated
[ ] release manifest generated
[ ] README updated
[ ] docs updated
[ ] compatibility matrix updated
[ ] changelog updated
[ ] examples executed
[ ] cloud build passed
[ ] cloud artifacts verified
[ ] local re-verification passed
[ ] Git clean
[ ] GitHub repository verified
[ ] v1.0.0 tag created
[ ] stable release verified
```

---

# 269. FINAL AUTOMATED RELEASE COMMAND

Implement:

```text
hcs release-verify
```

which runs the complete release gate.

The command must terminate with non-zero status on any critical failure.

---

# 270. FINAL AGENT BEHAVIOR

You are not finished when:

```text source code exists
```

You are finished when:

```text actual system works
```

You are not finished when:

```text API route exists
```

You are finished when:

```text real client request reaches the correct backend and returns the expected semantics
```

You are not finished when:

```text image code compiles
```

You are finished when:

```text a real image is generated and verified
```

You are not finished when:

```text dashboard renders
```

You are finished when:

```text live backend state is visible and UI actions control the real server
```

You are not finished when:

```text agent produces convincing prose
```

You are finished when:

```text actual tool execution + tests + verification prove the task
```

You are not finished when:

```text CI is green
```

You are finished when:

```text the produced artifact was actually verified
```

You are not finished when:

```text GitHub release exists
```

You are finished when:

```text the released artifact is locally reverified
```

---

# 271. FINAL ZERO-TO-HERO EXECUTION ORDER

Execute autonomously in this exact logical sequence:

```text
1. inspect current repository
2. inventory all files
3. inspect all existing models
4. inspect all supplied binaries
5. inspect current Git state
6. inspect existing GitHub remote
7. research current official API requirements
8. research current runtime requirements
9. research actual missing image dependencies
10. research actual OpenJev contract dependencies
11. identify all missing components
12. acquire missing required files
13. validate licenses
14. validate hashes
15. establish canonical directory structure
16. scaffold backend
17. scaffold dashboard
18. integrate daemon
19. integrate Prism Vulkan
20. integrate stable-diffusion.cpp Vulkan
21. integrate model registry
22. integrate model manager
23. integrate resource manager
24. integrate scheduler
25. implement OpenAI adapters
26. implement Anthropic adapters
27. implement HCS APIs
28. implement OpenJev contract
29. implement agent
30. implement tools
31. implement memory
32. implement telemetry
33. implement watchdog
34. implement recovery
35. implement dashboard
36. build locally
37. run real model smoke tests
38. run real OpenAI tests
39. run real Anthropic tests
40. run real OpenJev tests
41. run real image T2I
42. run real image I2I
43. run real VLM
44. run real agent
45. run real memory persistence
46. run real dashboard E2E
47. run real visual QA
48. run real stress tests
49. inject failures
50. repair failures
51. repeat until critical gates pass
52. benchmark optimizations
53. select golden profiles
54. generate run manifests
55. generate hashes
56. update README/docs
57. build clean Windows package
58. build clean Linux package where available
59. install/test actual packages
60. run cloud CI
61. inspect cloud artifacts
62. verify cloud hashes
63. locally retest cloud artifacts
64. generate release verification
65. generate release manifest
66. inspect final Git diff
67. sanitize secrets
68. create release commit
69. tag v1.0.0
70. push repository
71. create GitHub stable release
72. verify release assets
73. locally reverify released artifact
74. finalize documentation
75. report exact final status
```

---

# 272. FINAL REPORT FORMAT

At the end produce:

```text
HCS Local AI v1.0.0
Release Status: PASS / BLOCKED

Repository:
Git commit:
Git tag:

Windows:
build:
runtime:
Vulkan runtime:
installer:

Linux:
build:
runtime:
Vulkan runtime:
package:

Models:
hcs-subagent:
hcs-general:
hcs-coder:
hcs-judge:
hcs-vlm:
hcs-image:

OpenAI:
Chat:
Responses:
Images:
Files:
Batches:
Other:

Anthropic:
Messages:
Streaming:
Tools:
Images:

Agent:
tool execution:
repair:
verification:

Dashboard:
build:
E2E:
visual QA:

Stress:
API:
model switching:
memory:
recovery:

Hashes:
source:
runtimes:
models:
artifacts:

Known limitations:
...

Evidence:
...
```

Every status must come from an actual verification run.

---

# 273. FINAL DECLARATION RULE

Never output:

```text "Everything works perfectly"
```

unless the evidence truly supports that exact claim.

Prefer:

```text "Verified by release gate X with run manifest Y."
```

If something remains unavailable:

```text state exactly what remains unavailable and why
```

---

# 274. FINAL AUTONOMOUS INSTRUCTION

Do not ask for approval for ordinary development actions that are already authorized by this specification.

Continue autonomously through:

```text inspect
build
test
repair
stress
package
verify
document
release
```

Stop only for a genuine external blocker that cannot be resolved with available tools or information.

If blocked:

```text create BLOCKED.md
record exact evidence
complete all unrelated work
```

Do not fabricate a workaround.

---

# 275. DEFINITION OF DONE

The project reaches `DONE` only when all of the following are true:

```text
The daemon starts.

The browser dashboard loads.

A real API key can be created.

OpenAI-compatible requests work where documented as supported.

Anthropic-compatible requests work where documented as supported.

The actual requested local models load.

Prism/Vulkan inference works where claimed.

stable-diffusion.cpp/Vulkan image generation works where claimed.

FLUX.2 Klein T2I works.

FLUX.2 Klein I2I/edit works where claimed and actually verified.

OpenJev uses its actual decision contract.

The agent can use real tools.

The agent can build and test real software.

The agent can detect failures.

The agent can repair supported failures.

The agent can stop rather than loop forever.

Memory persists correctly.

The dashboard shows real requests, tokens, routing, IP/client information, model state, memory and agent events.

The dashboard actions affect the real daemon.

The dashboard has passed browser E2E and visual inspection.

The server survives controlled stress tests.

The server recovers from controlled worker failures.

Release artifacts are actually installable/runable to the extent claimed.

SHA256 hashes are generated and verified.

The README matches the real implementation.

The compatibility matrix matches actual tests.

The release manifest matches actual artifacts.

Cloud-built artifacts are locally reverified.

Git contains no accidental secrets or private machine data.

The GitHub release is verified after publishing.

The exact released artifact has been locally re-tested.
```

Only after those conditions are met may the implementation agent declare:

```text
HCS Local AI v1.0.0 — VERIFIED RELEASE
```

and publish the stable GitHub release.

---

# 276. HCS LOCAL AI v3.0.0 SPECIFICATION & MASTER PLAN INTEGRATION

The project lifecycle advances from v2.5.0 to **`HCS Local AI v3.0.0 Stable`**, incorporating the complete technical specifications detailed in `HCS_V3_MASTER_PLAN.md`:

## 276.1 CONTEXT COMPACTION & SUBAGENT ROLE
- Designate `hcs-subagent` (Bonsai 1.7B Q1_0) as the dedicated Platform Context Compactor.
- When context reaches $\ge 70\%$ of model window or exceeds 5,000 tokens, trigger automated 3-stage compaction:
  1. Extractive Pruning (strip verbose tool/shell outputs into hash anchors).
  2. Semantic State Synthesis (generate structured Context Delta: active goal, system state, recent discoveries, preserved code).
  3. Memory Graph Anchoring (deposit historical facts to SQLite WAL Brain).
- Guarantees $60-80\%$ context reduction with zero loss of active code context.

## 276.2 HARDWARE ACCELERATION & UMA SAFEGUARDS (AMD iGPU / VULKAN)
- Hardware Target: Windows 11 x64, AMD Radeon Graphics (UMA), 20–24 GB shared RAM.
- Strict single-heavy model concurrency: `max_heavy_active = 1` (`hcs-coder`, `hcs-vlm`, `hcs-image`).
- Vulkan worker tuning: Q8_0 KV Cache compression (`--cache-type-k q8_0 --cache-type-v q8_0`), Flash Attention on Vulkan (`-fa 1`), continuous batching (`--cont-batching`), thread pinning (8 compute threads, 8 OS reserved threads).
- Asynchronous worker eviction: Instant termination and virtual memory page de-commit ($< 400\text{ ms}$).
- Predictive Pre-warming: Concurrently initialize `hcs-coder` during Stage 1 intent extraction.

## 276.3 ADAPTIVE THINKING & WORKING TOKEN BUDGETS
- Dual-mode reasoning control:
  - Fast Mode: `enable_thinking = false`, direct token emission within 3–5 seconds for quick edits.
  - Deep Reasoning Mode: `enable_thinking = true`, thinking budget 512–1,536 tokens, working limit up to 4,096 tokens for architectural and SWE-bench tasks.
- Jev Decision Pipeline automatically switches modes based on complexity score ($\ge 4$ activates Deep Reasoning).

## 276.4 ADVANCED J-SPACE MULTI-MODEL WORKSPACE
- Persistent shared state container (`storage/jspace/<session_id>.json`) holding active repository path, git branches, linter diagnostics, and test traces.
- Seamless inter-model delegation: `subagent` (compaction/parsing) ➔ `judge` (OpenJev gate) ➔ `coder` (Bonsai 2-27B code patch) ➔ `judge`/`subagent` (verification) ➔ `brain` (learning).

## 276.5 NATIVE HCS AIDER AGENT & DASHBOARD STUDIO
- Dedicated repository-scale coding agent directly interfacing with `http://127.0.0.1:8787/v1`.
- Features: Tree-sitter repository tag map, atomic unified diff application (`SEARCH/REPLACE`), auto-rollback on test regression, git-backed checkpoints.
- Dashboard Studio: Modern SPA UI featuring interactive file tree, side-by-side diff review, live token monitor, and embedded terminal output.

## 276.6 CONTINUOUS BENCHMARK & PERSISTENT BRAIN LEARNING LOOP
- Expansion to 20-task benchmark pool (SWE-bench Lite, HumanEval+, Multi-File workloads).
- Autonomous self-healing feedback: failed compilation/test traces trigger patch generation and deposit `(failure_signature, corrective_diff)` into SQLite WAL store for future recall.

## 276.7 PACKAGING & WINDOWS INSTALLER (.exe)
- Creation of `HCS-Local-AI-v3.0.0-Setup.exe` (Inno Setup / NSIS).
- Bundles daemon binary, Web Dashboard SPA, Vulkan runtimes, HCS Aider Agent, 1-click `start.bat` / `stop.bat`, and Start Menu shortcuts.
- Fully verified without mock data before tag, release, and GitHub push.

