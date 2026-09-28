"""E2E Test for HCS Aider Autonomous Coding Agent.
Tests Tree-sitter repo mapping, atomic SEARCH/REPLACE diffing,
self-healing test loops, and SQLite Persistent Brain learning.
"""

import os
import sys
import shutil
import time
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT_DIR / "harnesses"))

from hcs_aider.agent import HCSAiderAgent

SANDBOX_DIR = ROOT_DIR / "benchmarks" / "aider_sandbox"


def setup_sandbox():
    if SANDBOX_DIR.exists():
        shutil.rmtree(SANDBOX_DIR, ignore_errors=True)
    SANDBOX_DIR.mkdir(parents=True, exist_ok=True)

    # Initial buggy code: LRU Cache where get() does not update entry position / access time
    buggy_code = '''"""LRUCache with Time-to-Live (TTL) eviction."""

import time

class LRUCache:
    def __init__(self, capacity: int = 2, ttl: float = 5.0):
        self.capacity = capacity
        self.ttl = ttl
        self.cache = {}
        self.timestamps = {}

    def get(self, key):
        if key not in self.cache:
            return None
        # BUG: Does not check expiration timestamp
        # BUG: Does not update access order in OrderedDict/cache
        return self.cache[key]

    def put(self, key, value):
        if len(self.cache) >= self.capacity and key not in self.cache:
            oldest = next(iter(self.cache))
            del self.cache[oldest]
            self.timestamps.pop(oldest, None)
        self.cache[key] = value
        self.timestamps[key] = time.time()
'''
    (SANDBOX_DIR / "cache.py").write_text(buggy_code, encoding="utf-8")

    test_code = '''import time
import pytest
from cache import LRUCache

def test_lru_eviction_and_expiration():
    c = LRUCache(capacity=2, ttl=0.1)
    c.put("a", 1)
    c.put("b", 2)

    # Access "a" so "b" becomes the least recently used
    assert c.get("a") == 1

    # Insert "c", which should evict "b"
    c.put("c", 3)
    assert c.get("a") == 1
    assert c.get("b") is None
    assert c.get("c") == 3

    # Test TTL expiration
    time.sleep(0.15)
    assert c.get("a") is None
    assert c.get("c") is None
'''
    (SANDBOX_DIR / "test_cache.py").write_text(test_code, encoding="utf-8")


def main():
    print("=" * 70)
    print(" HCS AIDER - Autonomous Coding Agent E2E Self-Healing Test")
    print("=" * 70)

    setup_sandbox()

    task = """In cache.py, LRUCache fails test_cache.py:
1. `get(key)` must check if `time.time() - self.timestamps[key] > self.ttl`. If expired, delete the key and return None.
2. In `get(key)`, accessing a valid key must move it to the end (most recently used). Use `collections.OrderedDict` for `self.cache` or pop and re-insert `self.cache[key]` so least recently used is evicted properly in `put`.
3. In `put(key, value)`, if key already exists, update its position and timestamp.
Provide SEARCH/REPLACE blocks to fix cache.py."""

    agent = HCSAiderAgent(
        repo_dir=SANDBOX_DIR,
        enable_thinking=False,
        max_tokens=1500,
        max_healing_turns=3,
    )

    test_cmd = f"{sys.executable} -m pytest -p no:langsmith -p no:cov -o addopts= test_cache.py -v"
    result = agent.execute_task_with_self_healing(task, test_cmd=test_cmd)

    print("\n" + "=" * 70)
    print(f" RESULT: {'SUCCESS' if result['success'] else 'FAILURE'}")
    print(f" Elapsed: {result['elapsed_seconds']:.2f}s | Turns: {result['turns']}")
    print("=" * 70)

    if not result["success"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
