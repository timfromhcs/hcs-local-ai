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

    def request(self, method, path, data=None, headers=None, timeout=60):
        url = f"{BASE_URL}{path}"
        all_headers = {"User-Agent": "HCS-Stress-Test"}
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
print("              HCS LOCAL AI v1.0.0 — COMPREHENSIVE STRESS TEST         ")
print("======================================================================")

# 1. Health & Doctor API
status, body, elapsed, err = ts.request("GET", "/hcs/v1/system")
ts.log_result("1. GET /hcs/v1/system", status == 200 and b"healthy" in body, f"({elapsed:.3f}s)")

# 2. Web Dashboard HTML
status, body, elapsed, err = ts.request("GET", "/")
ts.log_result("2. GET / (Dashboard HTML)", status == 200 and b"HCS Local AI" in body, f"({elapsed:.3f}s, {len(body)} bytes)")

# 3. Model Listing (OpenAI & HCS)
status, body, elapsed, err = ts.request("GET", "/v1/models")
data = json.loads(body.decode()) if status == 200 else {}
model_count = len(data.get("data", []))
ts.log_result("3. GET /v1/models", status == 200 and model_count >= 6, f"({model_count} models listed)")

status, body, elapsed, err = ts.request("GET", "/hcs/v1/models")
ts.log_result("4. GET /hcs/v1/models", status == 200 and b"hcs-subagent" in body, f"({elapsed:.3f}s)")

# 4. API Key Lifecycle
status, body, elapsed, err = ts.request("POST", "/hcs/v1/keys", {"name": "stress-test-key", "permissions": ["*"]})
key_data = json.loads(body.decode()) if status == 200 else {}
key_id = key_data.get("key", {}).get("id")
raw_key = key_data.get("raw_key") or key_data.get("raw_secret_key")
ts.log_result("5. POST /hcs/v1/keys (Create API Key)", status == 200 and raw_key is not None, f"(key_id: {key_id})")

status, body, elapsed, err = ts.request("GET", "/hcs/v1/keys")
ts.log_result("6. GET /hcs/v1/keys (List API Keys)", status == 200 and key_id.encode() in body if key_id else False)

# 5. Persistent Memory CRUD
status, body, elapsed, err = ts.request("POST", "/hcs/v1/memory", {
    "category": "stress_test",
    "key": "vulkan_perf",
    "content": "AMD Radeon iGPU running Vulkan unified memory",
    "provenance": "stress_suite"
})
mem_data = json.loads(body.decode()) if status == 200 else {}
mem_id = mem_data.get("id")
ts.log_result("7. POST /hcs/v1/memory (Insert)", status == 200 and mem_id is not None)

status, body, elapsed, err = ts.request("GET", "/hcs/v1/memory?q=Vulkan")
ts.log_result("8. GET /hcs/v1/memory?q=Vulkan (Search)", status == 200 and b"vulkan_perf" in body)

if mem_id:
    status, body, elapsed, err = ts.request("DELETE", f"/hcs/v1/memory/{mem_id}")
    ts.log_result("9. DELETE /hcs/v1/memory/{id} (Delete)", status == 200 and b'"deleted":true' in body)

# 6. OpenAI Chat Completions (Normal)
status, body, elapsed, err = ts.request("POST", "/v1/chat/completions", {
    "model": "hcs-subagent",
    "messages": [{"role": "user", "content": "What is 2+2? Answer in one word."}],
    "max_tokens": 8,
    "temperature": 0.0
}, timeout=90)
chat_data = json.loads(body.decode()) if status == 200 else {}
reply = chat_data.get("choices", [{}])[0].get("message", {}).get("content", "")
ts.log_result("10. POST /v1/chat/completions (Non-streaming)", status == 200 and len(reply) > 0, f"('{reply.strip()}' in {elapsed:.2f}s)")

# 7. OpenAI Chat Completions (Streaming SSE)
status, body, elapsed, err = ts.request("POST", "/v1/chat/completions", {
    "model": "hcs-subagent",
    "messages": [{"role": "user", "content": "Say 'hello world'"}],
    "max_tokens": 8,
    "stream": True
}, timeout=90)
has_chunks = b"data:" in body and b"[DONE]" in body
ts.log_result("11. POST /v1/chat/completions (Streaming SSE)", status == 200 and has_chunks, f"({len(body)} bytes in {elapsed:.2f}s)")

# 8. Anthropic Messages API (Normal)
status, body, elapsed, err = ts.request("POST", "/v1/messages", {
    "model": "hcs-subagent",
    "messages": [{"role": "user", "content": "Return 'verified'"}],
    "max_tokens": 16
}, headers={"x-api-key": "test"}, timeout=90)
anth_data = json.loads(body.decode()) if status == 200 else {}
content_blocks = anth_data.get("content", [])
ts.log_result("12. POST /v1/messages (Anthropic)", status == 200 and len(content_blocks) > 0, f"({elapsed:.2f}s)")

# 9. Anthropic Token Count
status, body, elapsed, err = ts.request("POST", "/v1/messages/count_tokens", {
    "messages": [{"role": "user", "content": "One two three four five six"}]
})
ts.log_result("13. POST /v1/messages/count_tokens", status == 200 and b"input_tokens" in body)

# 10. OpenJev Decision Contract (Strict deterministic evaluation)
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
sel_label = jev_data.get("selected_label")
sel_id = jev_data.get("selected_id") or jev_data.get("candidate_id")
ts.log_result("14. POST /hcs/v1/decision (OpenJev)", status == 200 and sel_label in ['A', 'B', 'C'], f"(Label {sel_label} -> {sel_id} in {elapsed:.2f}s)")

# 11. Files API (Local Artifacts)
status, body, elapsed, err = ts.request("POST", "/v1/files", {
    "filename": "stress_artifact.txt",
    "content": "HCS local artifact content for stress testing"
})
file_data = json.loads(body.decode()) if status == 200 else {}
file_id = file_data.get("id")
ts.log_result("15. POST /v1/files (Upload File)", status == 200 and file_id is not None)

if file_id:
    status, body, elapsed, err = ts.request("GET", f"/v1/files/{file_id}")
    ts.log_result("16. GET /v1/files/{id} (Metadata)", status == 200 and file_id.encode() in body)

    status, body, elapsed, err = ts.request("GET", f"/v1/files/{file_id}/content")
    ts.log_result("17. GET /v1/files/{id}/content (Download)", status == 200 and b"stress testing" in body)

    status, body, elapsed, err = ts.request("DELETE", f"/v1/files/{file_id}")
    ts.log_result("18. DELETE /v1/files/{id} (Delete)", status == 200 and b'"deleted":true' in body)

# 12. Batch API
status, body, elapsed, err = ts.request("POST", "/v1/batches", {
    "endpoint": "/v1/chat/completions",
    "completion_window": "24h",
    "requests": [{"custom_id": "req-1", "method": "POST", "url": "/v1/chat/completions", "body": {}}]
})
batch_data = json.loads(body.decode()) if status == 200 else {}
batch_id = batch_data.get("id")
ts.log_result("19. POST /v1/batches (Create Batch)", status == 200 and batch_id is not None)

if batch_id:
    status, body, elapsed, err = ts.request("GET", f"/v1/batches/{batch_id}")
    ts.log_result("20. GET /v1/batches/{id} (Status)", status == 200 and b"completed" in body or b"pending" in body)

# 13. Autonomous Agent Multi-Turn Tool Execution
status, body, elapsed, err = ts.request("POST", "/hcs/v1/agent/run", {
    "prompt": "Use command_exec to run 'cmd /c echo STRESS_TEST_OK' and report the output.",
    "model": "hcs-subagent"
}, timeout=90)
agent_data = json.loads(body.decode()) if status == 200 else {}
agent_status = agent_data.get("status")
steps = len(agent_data.get("steps_taken", []))
ts.log_result("21. POST /hcs/v1/agent/run (Autonomous Tool Exec)", status == 200 and agent_status == "completed", f"({steps} steps in {elapsed:.2f}s)")

# 14. Telemetry & Request Tracing
status, body, elapsed, err = ts.request("GET", "/hcs/v1/telemetry")
ts.log_result("22. GET /hcs/v1/telemetry (Realtime Metrics)", status == 200 and b"total_requests" in body)

status, body, elapsed, err = ts.request("GET", "/hcs/v1/requests")
ts.log_result("23. GET /hcs/v1/requests (SQLite Trace History)", status == 200 and b"requests" in body)

# 15. Fault Injection & Error Handling
status, body, elapsed, err = ts.request("POST", "/v1/chat/completions", "INVALID_NON_JSON")
ts.log_result("24. Fault Injection: Malformed JSON", status in [400, 415, 422], f"(Returned {status} gracefully)")

status, body, elapsed, err = ts.request("GET", "/nonexistent/endpoint/404")
ts.log_result("25. Fault Injection: Invalid 404 Route", status == 404, f"(Returned 404 correctly)")

# 16. Concurrency Stress Test (8 parallel requests)
print("\n--- Concurrency Stress Test: 8 Parallel Inferences ---")
def parallel_chat(idx):
    return ts.request("POST", "/v1/chat/completions", {
        "model": "hcs-subagent",
        "messages": [{"role": "user", "content": f"Compute {idx} * 2 and output only the number."}],
        "max_tokens": 8,
        "temperature": 0.0
    }, timeout=60)

t0 = time.time()
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
    concurrent_results = list(executor.map(parallel_chat, range(8)))
total_time = time.time() - t0

all_200 = all(r[0] == 200 for r in concurrent_results)
avg_lat = sum(r[2] for r in concurrent_results) / len(concurrent_results)
ts.log_result("26. Concurrency: 8 Simultaneous Chat Completions", all_200, f"(Total: {total_time:.2f}s, Avg per req: {avg_lat:.2f}s)")

# 17. Unload Model API
status, body, elapsed, err = ts.request("POST", "/hcs/v1/models/hcs-subagent/unload")
ts.log_result("27. POST /hcs/v1/models/:id/unload", status == 200 and b"unloaded" in body)

print("\n======================================================================")
print(f"STRESS TEST SUMMARY: {ts.passed} PASSED / {ts.failed} FAILED (Total: {len(ts.results)})")
print("======================================================================")

if ts.failed > 0:
    sys.exit(1)
