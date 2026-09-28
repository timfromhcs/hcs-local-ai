import json
import logging
import os
import re
import time
from typing import Any, Literal

import requests
from pydantic import BaseModel

from minisweagent.exceptions import FormatError
from minisweagent.models import GLOBAL_MODEL_STATS
from minisweagent.models.utils.actions_toolcall import (
    BASH_TOOL,
    format_toolcall_observation_messages,
    parse_toolcall_actions,
)
from minisweagent.models.utils.retry import retry

logger = logging.getLogger("hcs_model")


class HCSModelConfig(BaseModel):
    model_name: str = "hcs-coder"
    base_url: str = os.getenv("HCS_BASE_URL", "http://127.0.0.1:8787/v1")
    api_key: str = os.getenv("HCS_API_KEY", "sk-hcs-local")
    model_kwargs: dict[str, Any] = {}
    cost_tracking: Literal["default", "ignore_errors"] = "ignore_errors"
    format_error_template: str = "{{ error }}"
    observation_template: str = (
        "{% if output.exception_info %}<exception>{{output.exception_info}}</exception>\n{% endif %}"
        "<returncode>{{output.returncode}}</returncode>\n<output>\n{{output.output}}</output>"
    )
    multimodal_regex: str = ""


class HCSAPIError(Exception):
    """Custom exception for HCS API errors."""


class HCSModel:
    abort_exceptions: list[type[Exception]] = [KeyboardInterrupt, HCSAPIError]

    def __init__(self, **kwargs):
        self.config = HCSModelConfig(**kwargs)
        self._endpoint = f"{self.config.base_url.rstrip('/')}/chat/completions"

    def _query(self, messages: list[dict[str, Any]], **kwargs) -> dict:
        headers = {
            "Authorization": f"Bearer {self.config.api_key}",
            "Content-Type": "application/json",
        }
        # Filter messages for clean schema
        clean_msgs = []
        for m in messages:
            msg_dict = {"role": m.get("role", "user")}
            if "content" in m and m["content"] is not None:
                msg_dict["content"] = str(m["content"])
            if "tool_calls" in m:
                msg_dict["tool_calls"] = m["tool_calls"]
            if "tool_call_id" in m:
                msg_dict["tool_call_id"] = m["tool_call_id"]
            if "name" in m:
                msg_dict["name"] = m["name"]
            clean_msgs.append(msg_dict)

        max_tokens = self.config.model_kwargs.get("max_tokens", kwargs.get("max_tokens", 750))
        payload = {
            "model": self.config.model_name,
            "messages": clean_msgs,
            "temperature": self.config.model_kwargs.get("temperature", 0.1),
            "max_tokens": max_tokens,
            "chat_template_kwargs": self.config.model_kwargs.get("chat_template_kwargs", {"enable_thinking": False}),
        }
        if self.config.model_kwargs.get("use_tools", kwargs.get("use_tools", False)):
            payload["tools"] = [BASH_TOOL]

        try:
            resp = requests.post(self._endpoint, headers=headers, json=payload, timeout=360)
            resp.raise_for_status()
            return resp.json()
        except requests.exceptions.RequestException as e:
            raise HCSAPIError(f"HCS API request failed: {e}") from e

    def _parse_actions(self, response: dict) -> list[dict]:
        choices = response.get("choices", [])
        if not choices:
            raise FormatError({
                "role": "user",
                "content": "Empty choices in response from model.",
                "extra": {"interrupt_type": "FormatError"}
            })

        msg = choices[0].get("message", {})
        tool_calls = msg.get("tool_calls", [])

        if tool_calls:
            # Parse OpenAI tool call format
            actions = []
            for tc in tool_calls:
                fn = tc.get("function", {})
                args_raw = fn.get("arguments", "{}")
                if isinstance(args_raw, str):
                    try:
                        args = json.loads(args_raw)
                    except Exception:
                        args = {"command": args_raw}
                else:
                    args = args_raw
                actions.append({"command": args.get("command", "")})
            return actions

        content = msg.get("content", "") or ""
        # Fallback 1: ```mswea_bash_command ... ```
        m = re.findall(r"```(?:mswea_bash_command|bash|sh|cmd|powershell)?\s*\n(.*?)\n```", content, re.DOTALL)
        if m:
            return [{"command": cmd.strip()} for cmd in m[:1]]

        # Fallback 2: Check for COMPLETE_TASK command
        if "COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT" in content:
            return [{"command": "echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT"}]

        # If model gave pure command without backticks
        first_line = content.strip().splitlines()[0] if content.strip() else ""
        if first_line.startswith(("python ", "git ", "pytest ", "echo ", "cat ", "ls ", "dir ")):
            return [{"command": first_line}]

        if self.config.model_kwargs.get("strict_bash_action", False):
            raise FormatError({
                "role": "user",
                "content": "Please provide an action using either the bash tool or enclosed in a bash code block.",
                "extra": {"interrupt_type": "FormatError", "model_response": content}
            })
        return []

    def query(self, messages: list[dict[str, Any]], **kwargs) -> dict:
        for attempt in retry(logger=logger, abort_exceptions=self.abort_exceptions):
            with attempt:
                raw_response = self._query(messages, **kwargs)

        actions = self._parse_actions(raw_response)
        choice_msg = raw_response["choices"][0]["message"]
        message = dict(choice_msg)
        message["extra"] = {
            "actions": actions,
            "response": raw_response,
            "cost": 0.0,
            "timestamp": time.time(),
        }
        GLOBAL_MODEL_STATS.add(0.0)
        return message

    def format_observation_messages(
        self,
        outputs: list[dict],
        *,
        template_vars: dict | None = None,
    ) -> list[dict]:
        return format_toolcall_observation_messages(
            outputs,
            observation_template=self.config.observation_template,
            template_vars=template_vars,
            multimodal_regex=self.config.multimodal_regex,
        )
