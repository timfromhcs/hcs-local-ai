"""HCS Aider - Core Autonomous Coding Agent.
Implements real-time SSE streaming, smart model routing (hcs-coder, hcs-general, hcs-judge, hcs-vlm),
self-healing test & repair loop, J-Space coordination, Tree-sitter repo map prompting,
and SQLite Brain memory learning.
"""

import os
import sys
import time
import json
import subprocess
from pathlib import Path
from typing import Any, Optional, Callable

import requests

from .diff_applier import DiffApplier, DiffApplyError
from .repo_map import RepoMap


class HCSAiderAgent:
    def __init__(
        self,
        repo_dir: str | Path,
        base_url: str = "http://127.0.0.1:8787/v1",
        hcs_api_url: str = "http://127.0.0.1:8787/hcs/v2",
        model: str = "auto",
        enable_thinking: bool = False,
        max_tokens: int = 2048,
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
                timeout=3,
            )
            if r.status_code == 200:
                data = r.json()
                self.session_id = data.get("id")
                # Record initial variables
                requests.post(
                    f"{self.hcs_api_url}/jspace/sessions/{self.session_id}/variables",
                    json={"key": "active_repo", "value": str(self.repo_dir)},
                    timeout=3,
                )
        except Exception:
            self.session_id = f"local-session-{int(time.time())}"
        return self.session_id

    def smart_route_model(self, prompt: str, target_files: Optional[list[str]] = None) -> tuple[str, str, str]:
        """Intelligently select the optimal specialist model based on prompt characteristics.
        
        Returns: (model_name, category, reason)
        """
        if self.model and self.model != "auto":
            return self.model, "explicit", f"Manually configured override: {self.model}"

        p_lower = prompt.lower().strip()

        # 1. Vision intent
        if any(ext in p_lower for ext in [".png", ".jpg", ".jpeg", ".webp"]) or any(k in p_lower for k in ["screenshot", "bild", "image", "visual", "ui inspection"]):
            return "hcs-vlm", "multimodal", "Visual understanding & image inspection requested (Qwen 3.5-9B VLM)"

        # 2. Decision / Critique / Rating / Judgement intent
        if any(k in p_lower for k in ["entscheide", "judge", "rate", "compare", "evaluate", "review", "vergleiche", "welche option", "prioritise", "decide"]):
            return "hcs-judge", "decision_gate", "Decision contract & evaluation gate requested (OpenJev 4B in J-Space)"

        # 3. Conversational / Q&A / Architecture Explanation
        qa_keywords = [
            "was ist", "wie geht", "wie funktioniert", "erkläre", "warum", "wer bist du", "hallo", "hi", "help", "hilfe",
            "what is", "how does", "explain", "why", "describe", "summarize", "übersicht", "zusammenfassung", "kannst du", "zeig mir", "list", "show me", "was macht"
        ]
        code_keywords = [
            "fix", "bug", "implement", "schreibe", "ändere", "modify", "refactor", "erstelle datei", "create file", "lösche", "delete",
            "patch", "diff", "unittest", "pytest", "function", "class", "fehler", "error", "traceback", "repair", "repariere", "add", "hinzufügen"
        ]

        is_qa = any(k in p_lower for k in qa_keywords)
        is_code = any(k in p_lower for k in code_keywords) or (target_files and len(target_files) > 0)

        if is_code:
            return "hcs-coder", "code_engineering", "Complex code modification, multi-file refactoring, or bugfix (Bonsai 2-27B)"
        elif is_qa:
            return "hcs-general", "general_reasoning", "Conversational Q&A / architecture explanation (Ternary-Bonsai-4B)"
        else:
            if len(prompt.split()) > 15:
                return "hcs-coder", "code_engineering", "In-depth engineering analysis & code generation (Bonsai 2-27B)"
            return "hcs-general", "general_reasoning", "Fast general reasoning & explanation (Ternary-Bonsai-4B)"

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
1. When asked to modify code, output file changes using exact SEARCH/REPLACE blocks formatted like this:
path/to/file.ext
<<<<<<< SEARCH
[exact lines to replace]
=======
[replacement lines]
>>>>>>> REPLACE

2. Be concise, preserve existing indentation, and modify only necessary lines.
3. When creating a new file, leave the SEARCH block empty.
4. When asked general questions, explain clearly without unnecessary SEARCH/REPLACE blocks.
"""
        if target_files:
            prompt += f"\nActive files under focus: {', '.join(target_files)}\n"
        return prompt

    def query_model_stream(
        self,
        user_prompt: str,
        extra_messages: Optional[list[dict]] = None,
        target_files: Optional[list[str]] = None,
        on_token: Optional[Callable[[str], None]] = None,
        on_start: Optional[Callable[[], None]] = None,
        model_override: Optional[str] = None,
    ) -> dict[str, Any]:
        """Query local HCS daemon via SSE streaming with real-time token delivery and performance telemetry."""
        if not self.messages:
            self.messages.append({"role": "system", "content": self.build_system_prompt(target_files)})

        self.messages.append({"role": "user", "content": user_prompt})
        if extra_messages:
            self.messages.extend(extra_messages)

        # Context compaction check
        char_count = sum(len(str(m.get("content", ""))) for m in self.messages)
        if char_count > 120000:
            try:
                comp_resp = requests.post(
                    f"{self.hcs_api_url}/compact",
                    json={"messages": self.messages, "preserve_recent": 4},
                    timeout=10,
                )
                if comp_resp.status_code == 200:
                    self.messages = comp_resp.json().get("compacted_messages", self.messages)
            except Exception:
                pass

        # Smart Model Selection
        selected_model = model_override or self.smart_route_model(user_prompt, target_files)[0]

        payload = {
            "model": selected_model,
            "messages": self.messages,
            "temperature": 0.1,
            "max_tokens": self.max_tokens,
            "stream": True,
            "chat_template_kwargs": {"enable_thinking": self.enable_thinking},
        }

        full_content = []
        token_count = 0
        t0 = time.time()
        ttft = 0.0
        first_token_received = False
        timings = {}

        try:
            resp = requests.post(
                f"{self.base_url}/chat/completions",
                json=payload,
                headers={"x-hcs-compact": "auto", "Accept": "text/event-stream"},
                stream=True,
                timeout=360,
            )
            resp.raise_for_status()

            for line in resp.iter_lines():
                if not line:
                    continue
                line_str = line.decode("utf-8").strip()
                if line_str == "data: [DONE]":
                    break
                if line_str.startswith("data: "):
                    data_json_str = line_str[6:]
                    try:
                        chunk = json.loads(data_json_str)
                    except json.JSONDecodeError:
                        continue

                    if "timings" in chunk:
                        timings = chunk["timings"]

                    choices = chunk.get("choices", [])
                    if choices:
                        delta = choices[0].get("delta", {})
                        token = delta.get("content")
                        if token:
                            if not first_token_received:
                                first_token_received = True
                                ttft = time.time() - t0
                                if on_start:
                                    on_start()
                            full_content.append(token)
                            token_count += 1
                            if on_token:
                                on_token(token)

        except Exception as e:
            # If streaming connection failed, record error
            err_msg = f"\n[Inference Error]: {e}"
            if on_token:
                on_token(err_msg)
            full_content.append(err_msg)

        total_duration = time.time() - t0
        gen_duration = total_duration - ttft if ttft > 0 else total_duration
        tps = (token_count / gen_duration) if gen_duration > 0 else 0.0

        complete_text = "".join(full_content)
        self.messages.append({"role": "assistant", "content": complete_text})

        return {
            "content": complete_text,
            "ttft": round(ttft, 2),
            "total_tokens": token_count,
            "duration_sec": round(total_duration, 2),
            "tps": round(tps, 1),
            "model": selected_model,
            "timings": timings,
        }

    def query_model(self, user_prompt: str, extra_messages: Optional[list[dict]] = None) -> str:
        """Non-streaming query compatibility wrapper."""
        result = self.query_model_stream(user_prompt, extra_messages=extra_messages)
        return result["content"]

    def run_tests(self, test_cmd: Optional[str] = None) -> tuple[bool, str]:
        """Execute test suite safely in target repository."""
        py_bin = f'"{sys.executable}"'
        if test_cmd:
            cmd = test_cmd
        elif (self.repo_dir / "pytest.ini").exists() or (self.repo_dir / "tests").exists():
            cmd = f"{py_bin} -m pytest -p no:langsmith -p no:cov -o addopts= -v"
        elif (self.repo_dir / "Cargo.toml").exists():
            cmd = "cargo test"
        else:
            cmd = f"{py_bin} -m unittest discover"

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
