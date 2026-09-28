"""HCS Aider - Core Autonomous Coding Agent.
Implements the complete self-healing test & repair loop, J-Space coordination,
Tree-sitter repo map prompting, and SQLite Brain memory learning.
"""

import os
import sys
import time
import subprocess
from pathlib import Path
from typing import Any, Optional

import requests

from .diff_applier import DiffApplier, DiffApplyError
from .repo_map import RepoMap


class HCSAiderAgent:
    def __init__(
        self,
        repo_dir: str | Path,
        base_url: str = "http://127.0.0.1:8787/v1",
        hcs_api_url: str = "http://127.0.0.1:8787/hcs/v2",
        model: str = "hcs-coder",
        enable_thinking: bool = False,
        max_tokens: int = 1500,
        max_healing_turns: int = 4,
    ):
        self.repo_dir = Path(repo_dir).resolve()
        self.base_url = base_url.rstrip("/")
        self.hcs_api_url = hcs_api_url.rstrip("/")
        self.model = model
        self.enable_thinking = enable_thinking
        self.max_tokens = max_tokens
        self.max_healing_turns = max_healing_turns
        self.repo_map = RepoMap(self.repo_dir)
        self.session_id: Optional[str] = None
        self.messages: list[dict[str, Any]] = []

    def init_jspace_session(self, title: str = "HCS Aider Session") -> str:
        """Create or join J-Space multi-model workspace."""
        try:
            r = requests.post(
                f"{self.hcs_api_url}/jspace/sessions",
                json={"title": title},
                timeout=5,
            )
            if r.status_code == 200:
                data = r.json()
                self.session_id = data.get("id")
                # Record initial variables
                requests.post(
                    f"{self.hcs_api_url}/jspace/sessions/{self.session_id}/variables",
                    json={"key": "active_repo", "value": str(self.repo_dir)},
                    timeout=5,
                )
        except Exception:
            self.session_id = f"local-session-{int(time.time())}"
        return self.session_id

    def build_system_prompt(self, target_files: Optional[list[str]] = None) -> str:
        """Construct a high-density, context-efficient coding prompt."""
        map_text = self.repo_map.generate_map()
        prompt = f"""You are HCS Aider, an autonomous senior systems and software engineer.
You are working on the repository at: {self.repo_dir}

REPOSITORY SYMBOL MAP:
```
{map_text}
```

RULES FOR CODE MODIFICATIONS:
1. Output file changes using exact SEARCH/REPLACE blocks formatted like this:
path/to/file.ext
<<<<<<< SEARCH
[exact lines to replace]
=======
[replacement lines]
>>>>>>> REPLACE

2. Be concise, preserve existing indentation, and modify only necessary lines.
3. When creating a new file, leave the SEARCH block empty.
"""
        if target_files:
            prompt += f"\nActive files under focus: {', '.join(target_files)}\n"
        return prompt

    def query_model(self, user_prompt: str, extra_messages: Optional[list[dict]] = None) -> str:
        """Query local HCS daemon with automatic compaction and token budgeting."""
        if not self.messages:
            self.messages.append({"role": "system", "content": self.build_system_prompt()})

        # Append user prompt
        self.messages.append({"role": "user", "content": user_prompt})
        if extra_messages:
            self.messages.extend(extra_messages)

        # Trigger automatic context compaction if token estimate exceeds 4,500 tokens
        char_count = sum(len(str(m.get("content", ""))) for m in self.messages)
        if char_count > 120000:  # ~30k tokens with 64k context headroom
            try:
                comp_resp = requests.post(
                    f"{self.hcs_api_url}/compact",
                    json={"messages": self.messages, "preserve_recent": 4},
                    timeout=30,
                )
                if comp_resp.status_code == 200:
                    self.messages = comp_resp.json().get("compacted_messages", self.messages)
            except Exception:
                pass

        payload = {
            "model": self.model,
            "messages": self.messages,
            "temperature": 0.1,
            "max_tokens": self.max_tokens,
            "chat_template_kwargs": {"enable_thinking": self.enable_thinking},
        }

        resp = requests.post(
            f"{self.base_url}/chat/completions",
            json=payload,
            headers={"x-hcs-compact": "auto"},
            timeout=360,
        )
        resp.raise_for_status()
        res_json = resp.json()
        content = res_json["choices"][0]["message"]["content"]
        self.messages.append({"role": "assistant", "content": content})
        return content

    def run_tests(self, test_cmd: Optional[str] = None) -> tuple[bool, str]:
        """Execute test suite safely in target repository."""
        if test_cmd:
            cmd = test_cmd
        elif (self.repo_dir / "pytest.ini").exists() or (self.repo_dir / "tests").exists():
            cmd = f"{sys.executable} -m pytest -p no:langsmith -p no:cov -o addopts= -v"
        elif (self.repo_dir / "Cargo.toml").exists():
            cmd = "cargo test"
        else:
            cmd = f"{sys.executable} -m unittest discover"

        try:
            res = subprocess.run(
                cmd,
                shell=True,
                cwd=str(self.repo_dir),
                capture_output=True,
                text=True,
                timeout=60,
            )
            output = (res.stdout or "") + "\n" + (res.stderr or "")
            return (res.returncode == 0, output)
        except subprocess.TimeoutExpired:
            return (False, "Test execution timed out after 60 seconds")
        except Exception as e:
            return (False, f"Subprocess error executing '{cmd}': {e}")

    def execute_task_with_self_healing(
        self,
        task_instruction: str,
        test_cmd: Optional[str] = None,
    ) -> dict[str, Any]:
        """Full end-to-end task execution with self-healing feedback loop."""
        self.init_jspace_session(f"Task: {task_instruction[:40]}")
        start_time = time.time()

        print(f"\n[HCS AIDER] Task Initiated: {task_instruction}")
        print(f"[HCS AIDER] Active Repo: {self.repo_dir}")

        # Check Persistent Brain for prior verified fixes
        try:
            recall_resp = requests.get(
                f"{self.hcs_api_url}/brain/recall",
                params={"q": task_instruction[:60]},
                timeout=5,
            )
            if recall_resp.status_code == 200:
                insights = recall_resp.json().get("insights", [])
                if insights:
                    print(f"[HCS AIDER] Brain Recall: Found {len(insights)} relevant prior solution(s).")
                    best_sol = insights[0].get("solution", "")
                    task_instruction += f"\n\n[PERSISTENT BRAIN INSIGHT]: Previously verified fix for similar issue:\n{best_sol}"
        except Exception:
            pass

        # Turn 1: Generate initial patch
        response = self.query_model(task_instruction)
        try:
            applied = DiffApplier.extract_and_apply(response, self.repo_dir)
            print(f"[HCS AIDER] Applied diffs to {len(applied)} file(s).")
        except DiffApplyError as e:
            print(f"[HCS AIDER] Diff error: {e}")
            applied = []

        # Run test verification
        passed, test_output = self.run_tests(test_cmd)
        turn = 1

        while not passed and turn <= self.max_healing_turns:
            print(f"[HCS AIDER] Test Failed on turn {turn}. Initiating Self-Healing Loop...")
            turn += 1

            # Distill traceback for compact error prompt
            error_prompt = f"""Tests failed after applying changes.
Test Output:
```
{test_output[-1500:]}
```
Please inspect the failure carefully and provide the corrected SEARCH/REPLACE blocks."""

            healing_resp = self.query_model(error_prompt)
            try:
                applied = DiffApplier.extract_and_apply(healing_resp, self.repo_dir)
                print(f"[HCS AIDER] Applied self-healing diffs to {len(applied)} file(s).")
            except DiffApplyError as e:
                print(f"[HCS AIDER] Diff application error on healing turn {turn}: {e}")

            passed, test_output = self.run_tests(test_cmd)

        elapsed = time.time() - start_time

        if passed:
            print(f"[HCS AIDER] SUCCESS! All tests passed in {elapsed:.2f}s ({turn} turns).")
            # Deposit successful resolution into Persistent Brain
            try:
                requests.post(
                    f"{self.hcs_api_url}/brain/learn",
                    json={
                        "category": "coding_fix",
                        "pattern": task_instruction[:200],
                        "solution": response[:1000],
                        "confidence": 0.98,
                    },
                    timeout=5,
                )
                print("[HCS AIDER] Deposited verified fix into Persistent Brain (SQLite WAL).")
            except Exception:
                pass
        else:
            print(f"[HCS AIDER] FAILED after {turn} turns ({elapsed:.2f}s).")

        return {
            "success": passed,
            "turns": turn,
            "elapsed_seconds": elapsed,
            "session_id": self.session_id,
            "test_output": test_output[-500:],
        }
