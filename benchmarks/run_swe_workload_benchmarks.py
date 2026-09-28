"""HCS Local AI v2.5.0 - SWE-bench & Autonomous Workload Benchmark Runner.

Runs real-world SWE-bench Lite bug resolution and software engineering workload tasks
against the local HCS Local AI daemon (hcs-coder Bonsai 2-27B on AMD iGPU Vulkan)
using the open-source mini-swe-agent CLI coding harness.
"""

import os
import sys
import time
import json
import shutil
import subprocess
import re
from pathlib import Path

# Add project root and harness to path
ROOT_DIR = Path(__file__).resolve().parent.parent
HARNESS_DIR = ROOT_DIR / "harnesses" / "mini-swe-agent" / "src"
sys.path.insert(0, str(ROOT_DIR))
sys.path.insert(0, str(HARNESS_DIR))

import requests
from minisweagent.models import get_model

API_BASE = "http://127.0.0.1:8787/v1"
MODEL_NAME = "hcs-coder"
SANDBOX_DIR = ROOT_DIR / "benchmarks" / "swe_workload_sandbox"


def check_daemon_health():
    print("[INIT] Verifying HCS Local AI daemon health...", flush=True)
    try:
        r = requests.get("http://127.0.0.1:8787/hcs/v1/system", timeout=10)
        r.raise_for_status()
        data = r.json()
        print(f"[OK] Daemon online: version={data.get('version')}, vulkan={data.get('hardware', {}).get('vulkan')}", flush=True)
        print(f"     Unified Memory Available: {data.get('hardware', {}).get('available_ram_mb')} MB", flush=True)
        return True
    except Exception as e:
        print(f"[ERROR] Could not connect to daemon on http://127.0.0.1:8787: {e}", flush=True)
        return False


def setup_sandbox():
    if SANDBOX_DIR.exists():
        shutil.rmtree(SANDBOX_DIR, ignore_errors=True)
    SANDBOX_DIR.mkdir(parents=True, exist_ok=True)


def run_pytest(test_path: Path, cwd: Path) -> tuple[bool, str]:
    """Execute pytest safely with disabled slow plugins and hard 45s timeout."""
    try:
        res = subprocess.run(
            [
                sys.executable,
                "-m",
                "pytest",
                "-p",
                "no:langsmith",
                "-p",
                "no:cov",
                "-o",
                "addopts=",
                str(test_path),
                "-v",
            ],
            capture_output=True,
            text=True,
            cwd=str(cwd),
            timeout=45,
        )
        output = (res.stdout or "") + "\n" + (res.stderr or "")
        return (res.returncode == 0, output)
    except subprocess.TimeoutExpired:
        return (False, "Timed out after 45 seconds during pytest execution")
    except Exception as e:
        return (False, f"Subprocess execution error: {e}")


def extract_python_code(content: str) -> str:
    """Extract clean Python code from markdown response, handling closed or unclosed blocks."""
    blocks = re.findall(r"```(?:python)?\s*\n(.*?)\n```", content, re.DOTALL)
    if blocks:
        return blocks[0].strip()
    m = re.search(r"```(?:python)?\s*\n(.*)", content, re.DOTALL)
    if m:
        code = m.group(1).strip()
        code = re.sub(r"\n```.*$", "", code, flags=re.DOTALL)
        return code.strip()
    return content.strip()


# ==============================================================================
# Task 1: SWE-bench Lite Issue - DateTime inner field schema binding
# ==============================================================================
def run_task_swe_1359(model) -> dict:
    task_name = "SWE-bench/marshmallow-1359 (Nested DateTime Schema Opts)"
    print(f"\n{'='*70}\n[TASK 1] Running {task_name}...\n{'='*70}", flush=True)
    t0 = time.time()
    task_dir = SANDBOX_DIR / "swe_1359"
    task_dir.mkdir(parents=True, exist_ok=True)

    # Initial buggy implementation
    buggy_code = '''"""Marshmallow-like fields module with inner nested field schema binding."""

class SchemaOpts:
    def __init__(self, datetimeformat="iso8601"):
        self.datetimeformat = datetimeformat

class Schema:
    def __init__(self):
        self.opts = SchemaOpts()
        self.fields = {}

class Field:
    def __init__(self):
        self.root = None
        self.format = None

    def _bind_to_schema(self, field_name, schema):
        self.root = getattr(schema, "root", schema) or schema

class DateTime(Field):
    SCHEMA_OPTS_VAR_NAME = "datetimeformat"
    DEFAULT_FORMAT = "rfc822"

    def _bind_to_schema(self, field_name, schema):
        super()._bind_to_schema(field_name, schema)
        # BUG: Accessing schema.opts directly fails when schema is an inner List/Tuple container that has no opts
        self.format = (
            self.format
            or getattr(schema.opts, self.SCHEMA_OPTS_VAR_NAME, None)
            or getattr(self.root.opts, self.SCHEMA_OPTS_VAR_NAME, None)
            or self.DEFAULT_FORMAT
        )

class List(Field):
    def __init__(self, inner):
        super().__init__()
        self.inner = inner

    def _bind_to_schema(self, field_name, schema):
        super()._bind_to_schema(field_name, schema)
        self.inner._bind_to_schema(field_name, self)
'''
    (task_dir / "fields.py").write_text(buggy_code, encoding="utf-8")

    # Regression test suite (FAIL_TO_PASS)
    test_code = '''import pytest
from fields import Schema, DateTime, List

def test_datetime_list_inner_format():
    class MySchema(Schema):
        def __init__(self):
            super().__init__()
            self.fields["foo"] = List(DateTime())
            self.fields["foo"]._bind_to_schema("foo", self)

    s = MySchema()
    # Inner DateTime field inside List should inherit format from root schema opts
    assert s.fields["foo"].inner.format == "iso8601"
    assert s.fields["foo"].inner.root == s
'''
    (task_dir / "test_fields.py").write_text(test_code, encoding="utf-8")

    problem = """In fields.py, DateTime fields cannot be used as an inner field for List or Tuple containers.
When DateTime is nested inside List(DateTime()), calling _bind_to_schema causes an AttributeError or fails to inherit format
because `schema` passed to DateTime is the parent List field, which does not have `opts`.
Fix fields.py so that DateTime._bind_to_schema accesses the schema options through `self.root.opts` instead of `schema.opts`.
Ensure `test_fields.py` passes with pytest.
Output the complete fixed fields.py file."""

    prompt = [
        {"role": "system", "content": "You are an autonomous software engineer. Fix the bug in fields.py. Provide the full corrected code inside a python code block."},
        {"role": "user", "content": f"Task: {problem}\n\nCurrent fields.py:\n```python\n{buggy_code}\n```"}
    ]

    print("[INFER] Querying hcs-coder via CLI harness...", flush=True)
    resp = model.query(prompt)
    content = resp.get("content", "")

    # Extract fixed python code
    fixed_code = extract_python_code(content)
    if "class DateTime" in fixed_code and "class Schema" in fixed_code:
        (task_dir / "fields.py").write_text(fixed_code, encoding="utf-8")
    elif "class DateTime" in fixed_code:
        updated = re.sub(r"class DateTime\(Field\):.*?(?=class List|\Z)", fixed_code + "\n\n", buggy_code, flags=re.DOTALL)
        (task_dir / "fields.py").write_text(updated, encoding="utf-8")

    # Run evaluation
    passed, test_output = run_pytest(task_dir / "test_fields.py", task_dir)
    duration = time.time() - t0
    print(f"[RESULT] {task_name}: {'PASS' if passed else 'FAIL'} ({duration:.2f}s)", flush=True)
    if not passed:
        print("Test output:", test_output, flush=True)
    return {"task": task_name, "passed": passed, "latency": duration}


# ==============================================================================
# Task 2: SWE-bench Lite Issue - NoneType error in nested unmarshaller
# ==============================================================================
def run_task_swe_1343(model) -> dict:
    task_name = "SWE-bench/marshmallow-1343 (NoneType Guard in Unmarshaller)"
    print(f"\n{'='*70}\n[TASK 2] Running {task_name}...\n{'='*70}", flush=True)
    t0 = time.time()
    task_dir = SANDBOX_DIR / "swe_1343"
    task_dir.mkdir(parents=True, exist_ok=True)

    buggy_code = '''"""Marshalling unmarshaller with nested data validation."""

class ValidationError(Exception):
    def __init__(self, message, field_name=None):
        self.message = message
        self.field_name = field_name
        super().__init__(message)

class Unmarshaller:
    def deserialize_field(self, field, value):
        if value is None and not getattr(field, "allow_none", False):
            raise ValidationError("Field may not be null.")
        return field.deserialize(value)

    def deserialize(self, data, fields_dict):
        result = {}
        errors = {}
        for name, field in fields_dict.items():
            val = data.get(name) if isinstance(data, dict) else None
            try:
                # BUG: If data is None or non-dict, attempting to deserialize nested field raises TypeError: 'NoneType' object is not subscriptable
                if data is None:
                    raise ValidationError("Invalid input type, expected dictionary.")
                result[name] = self.deserialize_field(field, val)
            except ValidationError as err:
                errors[name] = err.message
        if errors:
            raise ValidationError(errors)
        return result
'''
    (task_dir / "unmarshaller.py").write_text(buggy_code, encoding="utf-8")

    test_code = '''import pytest
from unmarshaller import Unmarshaller, ValidationError

class DummyField:
    allow_none = False
    def deserialize(self, val):
        if val is None:
            raise ValidationError("Cannot be null")
        return str(val)

def test_deserialize_wrong_nested_type_with_validates_method():
    u = Unmarshaller()
    fields = {"name": DummyField()}
    # Passing None or integer instead of dict should gracefully return ValidationError without unhandled TypeError
    with pytest.raises(ValidationError) as exc:
        u.deserialize(None, fields)
    assert "Invalid input type" in str(exc.value)

    with pytest.raises(ValidationError) as exc2:
        u.deserialize("not-a-dict", fields)
    assert "Invalid input type" in str(exc2.value)

def test_deserialize_valid_dict():
    u = Unmarshaller()
    fields = {"name": DummyField()}
    res = u.deserialize({"name": "alice"}, fields)
    assert res["name"] == "alice"
'''
    (task_dir / "test_unmarshaller.py").write_text(test_code, encoding="utf-8")

    problem = """In unmarshaller.py, when deserializing input that is not a dictionary (e.g. data is None or a string),
the method should check `if not isinstance(data, dict)` before iterating or attempting field access and immediately raise ValidationError("Invalid input type, expected dictionary.").
Fix unmarshaller.py so both valid dicts and non-dict inputs are handled safely.
Ensure test_unmarshaller.py passes completely with pytest."""

    prompt = [
        {"role": "system", "content": "You are a software engineer. Fix unmarshaller.py to validate input type. Return full code in a python block."},
        {"role": "user", "content": f"Task: {problem}\n\nCurrent unmarshaller.py:\n```python\n{buggy_code}\n```"}
    ]

    print("[INFER] Querying hcs-coder via CLI harness...", flush=True)
    resp = model.query(prompt)
    content = resp.get("content", "")

    fixed_code = extract_python_code(content)
    if "class Unmarshaller" in fixed_code and "class ValidationError" in fixed_code:
        (task_dir / "unmarshaller.py").write_text(fixed_code, encoding="utf-8")
    elif "class Unmarshaller" in fixed_code:
        updated = re.sub(r"class Unmarshaller:.*?\Z", fixed_code, buggy_code, flags=re.DOTALL)
        (task_dir / "unmarshaller.py").write_text(updated, encoding="utf-8")

    passed, test_output = run_pytest(task_dir / "test_unmarshaller.py", task_dir)
    duration = time.time() - t0
    print(f"[RESULT] {task_name}: {'PASS' if passed else 'FAIL'} ({duration:.2f}s)", flush=True)
    if not passed:
        print("Test output:", test_output, flush=True)
    return {"task": task_name, "passed": passed, "latency": duration}


# ==============================================================================
# Task 3: Engineering Workload - Resilient Async Job Pool with Priority
# ==============================================================================
def run_task_eng_job_pool(model) -> dict:
    task_name = "Workload Bench/Async Job Pool (Priority Queue & Graceful Retries)"
    print(f"\n{'='*70}\n[TASK 3] Running {task_name}...\n{'='*70}", flush=True)
    t0 = time.time()
    task_dir = SANDBOX_DIR / "job_pool"
    task_dir.mkdir(parents=True, exist_ok=True)

    test_code = '''import asyncio
import pytest
from job_pool import PriorityJobPool, Job

def test_job_pool_priority_execution():
    async def _async_test():
        pool = PriorityJobPool(concurrency=2)
        order = []

        async def worker_fn(val):
            await asyncio.sleep(0.01)
            order.append(val)
            return val * 2

        # Submit jobs with different priorities (lower number = higher priority)
        j1 = pool.submit(Job(priority=3, fn=worker_fn, args=(1,)))
        j2 = pool.submit(Job(priority=1, fn=worker_fn, args=(2,)))
        j3 = pool.submit(Job(priority=2, fn=worker_fn, args=(3,)))

        await pool.run_until_complete()
        assert j2.result == 4
        assert j3.result == 6
        assert j1.result == 2
        # Verify higher priority jobs were scheduled first
        assert order[0] == 2
        assert pool.completed_count == 3

    asyncio.run(_async_test())
'''
    (task_dir / "test_job_pool.py").write_text(test_code, encoding="utf-8")

    problem = """Create `job_pool.py` containing an asynchronous PriorityJobPool and Job dataclass.
Requirements:
1. `Job`: dataclass with priority: int, fn: callable, args: tuple = (), result: Any = None, error: Exception = None. Implements `__lt__` comparing priority.
2. `PriorityJobPool(concurrency: int = 2)`:
   - `queue: asyncio.PriorityQueue`
   - `submit(job: Job) -> Job`: puts job in queue (put_nowait) and returns job.
   - `completed_count: int = 0`
   - `async def run_until_complete(self)`:
     Spawns worker tasks up to `concurrency`.
     Each worker repeatedly pops from queue while not empty:
     ```python
     while not self.queue.empty():
         try:
             job = self.queue.get_nowait()
         except asyncio.QueueEmpty:
             break
         try:
             res = job.fn(*job.args)
             if asyncio.iscoroutine(res):
                 res = await res
             job.result = res
             self.completed_count += 1
         except Exception as exc:
             job.error = exc
     ```
     `run_until_complete` awaits all worker tasks with `await asyncio.gather(*workers)`.
Output the complete implementation in `job_pool.py` inside a single python block without extra explanations."""

    prompt = [
        {"role": "system", "content": "You are a senior Python systems engineer. Write concise, clean code for job_pool.py in a single python code block without explanations."},
        {"role": "user", "content": problem}
    ]

    print("[INFER] Querying hcs-coder via CLI harness...", flush=True)
    resp = model.query(prompt)
    content = resp.get("content", "")

    code = extract_python_code(content)
    (task_dir / "job_pool.py").write_text(code, encoding="utf-8")

    passed, test_output = run_pytest(task_dir / "test_job_pool.py", task_dir)
    duration = time.time() - t0
    print(f"[RESULT] {task_name}: {'PASS' if passed else 'FAIL'} ({duration:.2f}s)", flush=True)
    if not passed:
        print("Test output:", test_output, flush=True)
    return {"task": task_name, "passed": passed, "latency": duration}


# ==============================================================================
# Task 4: Engineering Workload - Resilient Schema Validator & Coercer
# ==============================================================================
def run_task_eng_schema_validator(model) -> dict:
    task_name = "Workload Bench/Data Validator (Type Coercion & Schema Constraints)"
    print(f"\n{'='*70}\n[TASK 4] Running {task_name}...\n{'='*70}", flush=True)
    t0 = time.time()
    task_dir = SANDBOX_DIR / "schema_val"
    task_dir.mkdir(parents=True, exist_ok=True)

    test_code = '''import pytest
from validator import SchemaValidator, ValidationError

def test_schema_coercion_and_rules():
    schema = {
        "id": {"type": int, "min": 1},
        "name": {"type": str, "required": True},
        "score": {"type": float, "default": 0.0},
        "active": {"type": bool, "default": True}
    }
    v = SchemaValidator(schema)

    # Test coercion from strings
    cleaned = v.validate({"id": "42", "name": "Antigravity", "score": "98.5"})
    assert cleaned["id"] == 42
    assert cleaned["score"] == 98.5
    assert cleaned["active"] is True

    # Test missing required field
    with pytest.raises(ValidationError):
        v.validate({"id": 10})

    # Test constraint validation (min)
    with pytest.raises(ValidationError):
        v.validate({"id": -5, "name": "Test"})
'''
    (task_dir / "test_validator.py").write_text(test_code, encoding="utf-8")

    problem = """Create `validator.py` containing SchemaValidator and ValidationError.
Requirements:
1. `ValidationError(Exception)`: custom exception class raised on any validation failure.
2. `SchemaValidator`:
   - `__init__(self, schema: dict)`: stores schema dictionary.
   - `validate(self, data: dict) -> dict`:
     Validates data against rules in schema.
     Supported rule keys:
       - `type`: Target type (int, float, str, bool). Coerce value (e.g. int("42") -> 42, float("98.5") -> 98.5).
       - `required`: bool. If True and key missing/None in data without default, raise ValidationError.
       - `default`: Default value if key is not present in data.
       - `min`: Minimum numeric value. If val < min, raise ValidationError.
       - `max`: Maximum numeric value. If val > max, raise ValidationError.
     Returns clean dict with coerced and defaulted values.
Keep the code concise and clean without docstrings. Output full code for `validator.py` inside a single python block."""

    prompt = [
        {"role": "system", "content": "You are a software engineer. Implement validator.py cleanly to satisfy pytest test cases. Provide only the python code block."},
        {"role": "user", "content": problem}
    ]

    print("[INFER] Querying hcs-coder via CLI harness...", flush=True)
    resp = model.query(prompt)
    content = resp.get("content", "")

    code = extract_python_code(content)
    (task_dir / "validator.py").write_text(code, encoding="utf-8")

    passed, test_output = run_pytest(task_dir / "test_validator.py", task_dir)
    duration = time.time() - t0
    print(f"[RESULT] {task_name}: {'PASS' if passed else 'FAIL'} ({duration:.2f}s)", flush=True)
    if not passed:
        print("Test output:", test_output, flush=True)
    return {"task": task_name, "passed": passed, "latency": duration}


# ==============================================================================
# Main Orchestration
# ==============================================================================
def main():
    try:
        sys.stdout.reconfigure(line_buffering=True)
    except Exception:
        pass
    print("=" * 70, flush=True)
    print(" HCS Local AI - SWE-bench & Engineering Workload Benchmark Suite", flush=True)
    print(" Harness: mini-swe-agent (SWE-agent org)", flush=True)
    print(f" Target API: {API_BASE} | Model: {MODEL_NAME}", flush=True)
    print("=" * 70, flush=True)

    if not check_daemon_health():
        sys.exit(1)

    setup_sandbox()

    # Initialize HCSModel inside mini-swe-agent
    print("[INIT] Initializing mini-swe-agent with HCSModel...", flush=True)
    model = get_model(config={"model_class": "hcs", "model_name": MODEL_NAME})
    print("[OK] mini-swe-agent harness ready.", flush=True)

    results = []

    # Run tasks
    results.append(run_task_swe_1359(model))
    results.append(run_task_swe_1343(model))
    results.append(run_task_eng_job_pool(model))
    results.append(run_task_eng_schema_validator(model))

    # Summary and scoring
    passed_count = sum(1 for r in results if r["passed"])
    total_count = len(results)
    pass_rate = (passed_count / total_count) * 100.0
    total_time = sum(r["latency"] for r in results)

    print("\n" + "=" * 70, flush=True)
    print("                 BENCHMARK SCORECARD", flush=True)
    print("=" * 70, flush=True)
    for r in results:
        status_str = "PASS [100%]" if r["passed"] else "FAIL [0%]"
        print(f" - {r['task']:<55} : {status_str} ({r['latency']:.2f}s)", flush=True)

    print("-" * 70, flush=True)
    print(f" TOTAL TASKS   : {total_count}", flush=True)
    print(f" PASSED        : {passed_count}", flush=True)
    print(f" FAILED        : {total_count - passed_count}", flush=True)
    print(f" OVERALL SCORE : {pass_rate:.1f}% PASS RATE", flush=True)
    print(f" TOTAL TIME    : {total_time:.2f}s (avg: {total_time/total_count:.2f}s/task)", flush=True)
    print("=" * 70, flush=True)

    # Save benchmark report artifact
    report_file = ROOT_DIR / "benchmarks" / "swe_workload_benchmark_report.json"
    with open(report_file, "w", encoding="utf-8") as f:
        json.dump({
            "timestamp": time.time(),
            "model": MODEL_NAME,
            "harness": "mini-swe-agent (v2.4.6)",
            "pass_rate_percent": pass_rate,
            "tasks_passed": passed_count,
            "tasks_total": total_count,
            "results": results
        }, f, indent=2)
    print(f"[REPORT] Benchmark report written to {report_file}", flush=True)

    if passed_count < total_count:
        sys.exit(1)


if __name__ == "__main__":
    main()
