"""HCS Aider — Autonomous Code Editing & Pair Programming Benchmark.
Tests atomic SEARCH/REPLACE diff extraction, multi-file code modifications,
AST symbol mapping, and closed-loop self-healing against hcs-coder on AMD Vulkan.
"""

import os
import sys
import time
import json
import shutil
import subprocess
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "harnesses"))

from hcs_aider.agent import HCSAiderAgent
from hcs_aider.diff_applier import DiffApplier

BENCHMARK_SANDBOX = ROOT_DIR / "benchmarks" / "aider_benchmark_sandbox"


def setup_sandbox(sub_dir: str) -> Path:
    target = BENCHMARK_SANDBOX / sub_dir
    if target.exists():
        shutil.rmtree(target, ignore_errors=True)
    target.mkdir(parents=True, exist_ok=True)
    return target


def run_test_case_lru_cache() -> dict:
    task_name = "AiderBench/01 - LRU Cache with TTL & Access Order Eviction"
    print(f"\n{'='*70}\n[TASK 1] {task_name}\n{'='*70}")
    t0 = time.time()
    work_dir = setup_sandbox("lru_cache")

    # Buggy code
    buggy = '''import time

class LRUCache:
    def __init__(self, capacity: int = 2, ttl: float = 1.0):
        self.capacity = capacity
        self.ttl = ttl
        self.cache = {}
        self.timestamps = {}

    def get(self, key):
        if key not in self.cache:
            return None
        # BUG: Doesn't check TTL expiration
        # BUG: Doesn't refresh access order
        return self.cache[key]

    def put(self, key, value):
        if len(self.cache) >= self.capacity and key not in self.cache:
            oldest = next(iter(self.cache))
            del self.cache[oldest]
            self.timestamps.pop(oldest, None)
        self.cache[key] = value
        self.timestamps[key] = time.time()
'''
    (work_dir / "cache.py").write_text(buggy, encoding="utf-8")

    test_file = '''import time
from cache import LRUCache

def test_lru_eviction():
    c = LRUCache(capacity=2, ttl=0.2)
    c.put("a", 1)
    c.put("b", 2)
    assert c.get("a") == 1
    c.put("c", 3)
    assert c.get("b") is None
    assert c.get("a") == 1
    assert c.get("c") == 3

def test_ttl_expiration():
    c = LRUCache(capacity=2, ttl=0.1)
    c.put("k", 100)
    assert c.get("k") == 100
    time.sleep(0.15)
    assert c.get("k") is None
'''
    (work_dir / "test_cache.py").write_text(test_file, encoding="utf-8")

    task = """In cache.py, fix LRUCache so it passes test_cache.py:
1. In `get(key)`, verify expiration: if `time.time() - self.timestamps[key] > self.ttl`, delete key and return None.
2. In `get(key)`, accessing a key must update its access position so it is marked most recently used.
3. In `put(key, value)`, if key already exists, update its position and timestamp.
Output changes using SEARCH/REPLACE blocks."""

    agent = HCSAiderAgent(repo_dir=work_dir, enable_thinking=False, max_tokens=1500)
    test_cmd = f"{sys.executable} -m pytest -p no:langsmith -p no:cov -o addopts= test_cache.py -v"
    result = agent.execute_task_with_self_healing(task, test_cmd=test_cmd)

    dur = time.time() - t0
    return {
        "task": task_name,
        "passed": result["success"],
        "turns": result["turns"],
        "duration_seconds": dur,
    }


def run_test_case_rate_limiter() -> dict:
    task_name = "AiderBench/02 - Token Bucket Rate Limiter with Refill"
    print(f"\n{'='*70}\n[TASK 2] {task_name}\n{'='*70}")
    t0 = time.time()
    work_dir = setup_sandbox("rate_limiter")

    buggy = '''import time

class TokenBucket:
    def __init__(self, capacity: int, fill_rate: float):
        self.capacity = float(capacity)
        self.fill_rate = float(fill_rate)
        self.tokens = float(capacity)
        self.last_updated = time.time()

    def consume(self, tokens: int = 1) -> bool:
        # BUG: Never refills tokens based on elapsed time!
        if self.tokens >= tokens:
            self.tokens -= tokens
            return True
        return False
'''
    (work_dir / "limiter.py").write_text(buggy, encoding="utf-8")

    test_file = '''import time
from limiter import TokenBucket

def test_token_bucket_consume_and_refill():
    tb = TokenBucket(capacity=2, fill_rate=10.0) # 10 tokens per sec
    assert tb.consume(2) is True
    assert tb.consume(1) is False
    time.sleep(0.12) # ~1.2 tokens refilled
    assert tb.consume(1) is True
    assert tb.consume(1) is False
'''
    (work_dir / "test_limiter.py").write_text(test_file, encoding="utf-8")

    task = """In limiter.py, fix TokenBucket:
1. In `consume()`, calculate elapsed time `now - self.last_updated`.
2. Add `elapsed * self.fill_rate` to `self.tokens`, clamped to `self.capacity`.
3. Update `self.last_updated = now`.
4. If tokens >= requested, deduct and return True, else False.
Output SEARCH/REPLACE blocks."""

    agent = HCSAiderAgent(repo_dir=work_dir, enable_thinking=False, max_tokens=1500)
    test_cmd = f"{sys.executable} -m pytest -p no:langsmith -p no:cov -o addopts= test_limiter.py -v"
    result = agent.execute_task_with_self_healing(task, test_cmd=test_cmd)

    dur = time.time() - t0
    return {
        "task": task_name,
        "passed": result["success"],
        "turns": result["turns"],
        "duration_seconds": dur,
    }


def main():
    print("=" * 70)
    print("       HCS AIDER — AUTONOMOUS CODING BENCHMARK SUITE       ")
    print("=" * 70)

    # Check daemon health
    try:
        r = requests.get("http://127.0.0.1:8787/hcs/v1/system", timeout=5)
        if r.status_code != 200:
            print("[ERROR] Daemon reported unhealthy status.")
            sys.exit(1)
    except Exception as e:
        print(f"[ERROR] Cannot connect to HCS daemon at http://127.0.0.1:8787: {e}")
        print("Please start server with start.bat or hcs-daemon.exe first.")
        sys.exit(1)

    results = []
    results.append(run_test_case_lru_cache())
    results.append(run_test_case_rate_limiter())

    passed_count = sum(1 for r in results if r["passed"])
    total_count = len(results)
    pass_rate = (passed_count / total_count) * 100.0

    report = {
        "timestamp": time.time(),
        "benchmark": "HCS Aider Coding Benchmark",
        "model": "hcs-coder",
        "tasks_total": total_count,
        "tasks_passed": passed_count,
        "pass_rate_percent": pass_rate,
        "results": results,
    }

    report_path = ROOT_DIR / "benchmarks" / "aider_benchmark_report.json"
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")

    print("\n" + "=" * 70)
    print(f" FINAL BENCHMARK SCORE: {passed_count}/{total_count} PASSED ({pass_rate:.1f}%)")
    print(f" Report written to: {report_path}")
    print("=" * 70)

    if pass_rate < 100.0:
        sys.exit(1)


if __name__ == "__main__":
    main()
