import concurrent.futures
import json
import time
import urllib.request
import urllib.error
import sys

BASE_URL = "http://127.0.0.1:8787"

class TestSuite:
    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.results = []

    def log_result(self, name, success, details=""):
        if success:
            self.passed += 1
            print(f" [PASS] {name} {details}")
        else:
            self.failed += 1
            print(f" [FAIL] {name} - ERROR: {details}")
        self.results.append({"name": name, "success": success, "details": details})

    def request(self, method, path, data=None, headers=None, timeout=90):
        url = f"{BASE_URL}{path}"
        all_headers = {"User-Agent": "HCS-v2-Stress-Test"}
        if headers:
            all_headers.update(headers)
        
        req_data = None
        if data is not None:
            if isinstance(data, (dict, list)):
                req_data = json.dumps(data).encode("utf-8")
                all_headers["Content-Type"] = "application/json"
            elif isinstance(data, (str, bytes)):
                req_data = data.encode("utf-8") if isinstance(data, str) else data

        req = urllib.request.Request(url, data=req_data, headers=all_headers, method=method)
        t0 = time.time()
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                elapsed = time.time() - t0
                body = resp.read()
                return resp.status, body, elapsed, None
        except urllib.error.HTTPError as e:
            elapsed = time.time() - t0
            return e.code, e.read(), elapsed, str(e)
        except Exception as e:
            elapsed = time.time() - t0
            return 0, b"", elapsed, str(e)

ts = TestSuite()

print("======================================================================")
print("              HCS LOCAL AI v2.0.0 — COMPREHENSIVE STRESS TEST         ")
print("======================================================================")

# 1. System Health & v2 Version
status, body, elapsed, err = ts.request("GET", "/hcs/v1/system")
data = json.loads(body.decode()) if status == 200 else {}
ts.log_result("1. GET /hcs/v1/system (v2.0.0 Health)", status == 200 and data.get("version") == "2.0.0", f"({elapsed:.3f}s)")

# 2. Hardware Profile (Threads, Q8 KV Cache, Flash Attn, UMA)
status, body, elapsed, err = ts.request("GET", "/hcs/v2/hardware/profile")
hw = json.loads(body.decode()) if status == 200 else {}
valid_hw = (hw.get("cpu_threads") == 8 and 
            hw.get("kv_cache_k") == "q8_0" and 
            hw.get("vulkan_acceleration") is True)
ts.log_result("2. GET /hcs/v2/hardware/profile (Q8 KV Cache, Threads 8, Vulkan)", status == 200 and valid_hw, f"({elapsed:.3f}s)")

# 3. Web Dashboard HTML
status, body, elapsed, err = ts.request("GET", "/")
ts.log_result("3. GET / (Dashboard HTML v2)", status == 200 and b"v2.0.0" in body and b"J-Space" in body, f"({len(body)} bytes)")

# 4. Model Listing
status, body, elapsed, err = ts.request("GET", "/v1/models")
data = json.loads(body.decode()) if status == 200 else {}
model_count = len(data.get("data", []))
ts.log_result("4. GET /v1/models (6 Models Available)", status == 200 and model_count >= 6, f"({model_count} models)")

# 5. J-Space Session Creation
status, body, elapsed, err = ts.request("POST", "/hcs/v2/jspace/sessions", {
    "title": "Autonomous Concurrency Architecture",
    "initial_goal": "Benchmark Bonsai 2-27B on AMD iGPU"
})
jspace_data = json.loads(body.decode()) if status == 200 else {}
session_id = jspace_data.get("id")
ts.log_result("5. POST /hcs/v2/jspace/sessions (Create Session)", status == 200 and session_id is not None, f"(id: {session_id})")

# 6. J-Space Shared State & Turns
if session_id:
    status, body, elapsed, err = ts.request("POST", f"/hcs/v2/jspace/sessions/{session_id}/state", {
        "key": "target_backend",
        "value": "vulkan_uma"
    })
    ts.log_result("6. POST /hcs/v2/jspace/sessions/:id/state (Set Shared State)", status == 200 and b'"success":true' in body)

    status, body, elapsed, err = ts.request("POST", f"/hcs/v2/jspace/sessions/{session_id}/turns", {
        "role": "agent",
        "model": "hcs-coder",
        "content": "J-Space multi-model turn coordination verified"
    })
    ts.log_result("7. POST /hcs/v2/jspace/sessions/:id/turns (Append Turn)", status == 200 and b'"success":true' in body)

    status, body, elapsed, err = ts.request("GET", f"/hcs/v2/jspace/sessions/{session_id}")
    s_get = json.loads(body.decode()) if status == 200 else {}
    valid_session = (s_get.get("shared_state", {}).get("target_backend") == "vulkan_uma" and
                     len(s_get.get("turns", [])) >= 1)
    ts.log_result("8. GET /hcs/v2/jspace/sessions/:id (Fetch Session)", status == 200 and valid_session)

# 7. Persistent Brain Auto-Learning & Recall
status, body, elapsed, err = ts.request("POST", "/hcs/v2/brain/learn", {
    "category": "architecture",
    "pattern": "Memory limit exceeded during heavy compilation",
    "solution": "Throttle worker threads to half core count and enable Q8 KV cache",
    "confidence": 0.98
})
ts.log_result("9. POST /hcs/v2/brain/learn (Record Insight)", status == 200 and b"recorded_id" in body)

status, body, elapsed, err = ts.request("GET", "/hcs/v2/brain/recall?q=compilation")
recalled = json.loads(body.decode()) if status == 200 else {}
ts.log_result("10. GET /hcs/v2/brain/recall (Search Knowledge)", status == 200 and recalled.get("count", 0) >= 1)

# 8. Jev Plan Rating
status, body, elapsed, err = ts.request("POST", "/hcs/v2/jev/rate_plan", {
    "goal": "Refactor codebase cleanly",
    "plans": [
        "Plan A: Run atomic multi-file edits with unit test validation",
        "Plan B: Overwrite everything blindly"
    ]
}, timeout=90)
rate_data = json.loads(body.decode()) if status == 200 else {}
ts.log_result("11. POST /hcs/v2/jev/rate_plan (OpenJev Plan Rating)", status == 200 and rate_data.get("selected_plan") == "plan_1", f"({elapsed:.2f}s)")

# 9. Jev 3-Stage Smart Delegation Pipeline
status, body, elapsed, err = ts.request("POST", "/hcs/v2/jev/delegate", {
    "prompt": "Write a complex Rust SIMD Vulkan compute shader compiler and optimize kernel execution"
}, timeout=90)
del_data = json.loads(body.decode()) if status == 200 else {}
selected_model = del_data.get("selected_model")
ts.log_result("12. POST /hcs/v2/jev/delegate (Selects 27B for Heavy Coding)", status == 200 and selected_model == "hcs-coder", f"(Routed to {selected_model} in {elapsed:.2f}s)")

# 10. OpenJev Standard Decision Contract
status, body, elapsed, err = ts.request("POST", "/hcs/v1/decision", {
    "state": "High memory consumption detected on AMD iGPU",
    "instructions": "Select optimal memory mitigation action",
    "criteria": [
        {"id": "evict_idle", "description": "Evict idle cold workers to free unified memory"},
        {"id": "terminate_all", "description": "Kill all daemon processes"},
        {"id": "ignore", "description": "Do nothing"}
    ]
}, timeout=90)
jev_data = json.loads(body.decode()) if status == 200 else {}
ts.log_result("13. POST /hcs/v1/decision (Deterministic OpenJev)", status == 200 and jev_data.get("selected_label") in ['A', 'B', 'C'], f"({elapsed:.2f}s)")

# 11. OpenAI Chat Completions (Non-Streaming)
status, body, elapsed, err = ts.request("POST", "/v1/chat/completions", {
    "model": "hcs-subagent",
    "messages": [{"role": "user", "content": "What is 3+3? Answer with one number."}],
    "max_tokens": 8,
    "temperature": 0.0
}, timeout=90)
chat_data = json.loads(body.decode()) if status == 200 else {}
content = chat_data.get("choices", [{}])[0].get("message", {}).get("content", "")
ts.log_result("14. POST /v1/chat/completions (Non-streaming)", status == 200 and len(content) > 0, f"('{content.strip()}' in {elapsed:.2f}s)")

# 12. OpenAI Chat Completions (SSE Streaming)
status, body, elapsed, err = ts.request("POST", "/v1/chat/completions", {
    "model": "hcs-subagent",
    "messages": [{"role": "user", "content": "Say 'v2 ready'"}],
    "max_tokens": 8,
    "stream": True
}, timeout=90)
has_stream = b"data:" in body and b"[DONE]" in body
ts.log_result("15. POST /v1/chat/completions (Streaming SSE)", status == 200 and has_stream, f"({len(body)} bytes in {elapsed:.2f}s)")

# 13. Anthropic Messages & Token Count
status, body, elapsed, err = ts.request("POST", "/v1/messages", {
    "model": "hcs-subagent",
    "messages": [{"role": "user", "content": "Return 'verified'"}],
    "max_tokens": 16
}, headers={"x-api-key": "test"}, timeout=90)
ts.log_result("16. POST /v1/messages (Anthropic)", status == 200 and b"content" in body, f"({elapsed:.2f}s)")

status, body, elapsed, err = ts.request("POST", "/v1/messages/count_tokens", {
    "messages": [{"role": "user", "content": "Counting tokens for HCS v2"}]
})
ts.log_result("17. POST /v1/messages/count_tokens", status == 200 and b"input_tokens" in body)

# 14. Files API (Local Artifacts)
status, body, elapsed, err = ts.request("POST", "/v1/files", {
    "filename": "v2_test_artifact.txt",
    "content": "HCS v2 local artifact content"
})
file_data = json.loads(body.decode()) if status == 200 else {}
file_id = file_data.get("id")
ts.log_result("18. POST /v1/files (Upload Artifact)", status == 200 and file_id is not None)

if file_id:
    status, body, elapsed, err = ts.request("GET", f"/v1/files/{file_id}/content")
    ts.log_result("19. GET /v1/files/:id/content (Download)", status == 200 and b"v2 local artifact" in body)

    status, body, elapsed, err = ts.request("DELETE", f"/v1/files/{file_id}")
    ts.log_result("20. DELETE /v1/files/:id", status == 200 and b'"deleted":true' in body)

# 15. Batch API
status, body, elapsed, err = ts.request("POST", "/v1/batches", {
    "endpoint": "/v1/chat/completions",
    "completion_window": "24h",
    "requests": [{"custom_id": "req-v2", "method": "POST", "url": "/v1/chat/completions", "body": {}}]
})
batch_data = json.loads(body.decode()) if status == 200 else {}
batch_id = batch_data.get("id")
ts.log_result("21. POST /v1/batches (Create Batch)", status == 200 and batch_id is not None)

if batch_id:
    status, body, elapsed, err = ts.request("GET", f"/v1/batches/{batch_id}")
    ts.log_result("22. GET /v1/batches/:id", status == 200 and b"batch" in body or b"status" in body)

# 16. Autonomous Multi-Turn Agent Tool Execution
status, body, elapsed, err = ts.request("POST", "/hcs/v1/agent/run", {
    "prompt": "Use command_exec to run 'cmd /c echo V2_AGENT_OK' and report output.",
    "model": "hcs-subagent"
}, timeout=90)
agent_data = json.loads(body.decode()) if status == 200 else {}
ts.log_result("23. POST /hcs/v1/agent/run (Autonomous Agent Execution)", status == 200 and agent_data.get("status") == "completed", f"({elapsed:.2f}s)")

# 17. Telemetry & SQLite Request Tracing
status, body, elapsed, err = ts.request("GET", "/hcs/v1/telemetry")
ts.log_result("24. GET /hcs/v1/telemetry", status == 200 and b"total_requests" in body)

status, body, elapsed, err = ts.request("GET", "/hcs/v1/requests")
ts.log_result("25. GET /hcs/v1/requests", status == 200 and b"requests" in body)

# 18. Fault Injection
status, body, elapsed, err = ts.request("POST", "/v1/chat/completions", "INVALID_MALFORMED")
ts.log_result("26. Fault Injection: Malformed JSON", status in [400, 415, 422], f"(Returned {status} gracefully)")

status, body, elapsed, err = ts.request("GET", "/nonexistent/endpoint/404")
ts.log_result("27. Fault Injection: Invalid 404 Route", status == 404, f"(Returned 404 correctly)")

# 19. Concurrency Stress Test: 8 Simultaneous Chat Inferences
print("\n--- Concurrency Stress Test: 8 Parallel Inferences on Vulkan ---")
def parallel_chat(idx):
    return ts.request("POST", "/v1/chat/completions", {
        "model": "hcs-subagent",
        "messages": [{"role": "user", "content": f"Calculate {idx} + 10 and return only the number."}],
        "max_tokens": 8,
        "temperature": 0.0
    }, timeout=60)

t0 = time.time()
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
    concurrent_results = list(executor.map(parallel_chat, range(8)))
total_time = time.time() - t0

all_200 = all(r[0] == 200 for r in concurrent_results)
avg_lat = sum(r[2] for r in concurrent_results) / len(concurrent_results)
ts.log_result("28. Concurrency: 8 Simultaneous Requests", all_200, f"(Total: {total_time:.2f}s, Avg: {avg_lat:.2f}s)")

# 20. Clean Model Unload API
status, body, elapsed, err = ts.request("POST", "/hcs/v1/models/hcs-subagent/unload")
ts.log_result("29. POST /hcs/v1/models/:id/unload", status == 200 and b"unloaded" in body)

# 21. Delete J-Space Session
if session_id:
    status, body, elapsed, err = ts.request("DELETE", f"/hcs/v2/jspace/sessions/{session_id}")
    ts.log_result("30. DELETE /hcs/v2/jspace/sessions/:id", status == 200 and b'"deleted":true' in body)

print("\n======================================================================")
print(f"HCS v2.0.0 STRESS TEST SUMMARY: {ts.passed} PASSED / {ts.failed} FAILED (Total: {len(ts.results)})")
print("======================================================================")

if ts.failed > 0:
    sys.exit(1)
