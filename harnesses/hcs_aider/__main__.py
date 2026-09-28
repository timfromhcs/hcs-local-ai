"""CLI Entry Point for HCS Aider Autonomous Coding Agent."""

import argparse
import sys
from pathlib import Path
from .agent import HCSAiderAgent


def main():
    parser = argparse.ArgumentParser(description="HCS Aider - Native Autonomous Coding Agent")
    parser.add_argument("--dir", default=".", help="Target repository directory (default: current directory)")
    parser.add_argument("--task", required=True, help="Coding task or bugfix instruction")
    parser.add_argument("--test-cmd", default=None, help="Custom test command (e.g. 'pytest tests/test_core.py')")
    parser.add_argument("--thinking", action="store_true", help="Enable deep reasoning thinking mode")
    parser.add_argument("--max-tokens", type=int, default=1500, help="Maximum working generation tokens (default: 1500)")

    args = parser.parse_args()

    agent = HCSAiderAgent(
        repo_dir=args.dir,
        enable_thinking=args.thinking,
        max_tokens=args.max_tokens,
    )

    result = agent.execute_task_with_self_healing(args.task, test_cmd=args.test_cmd)
    if result["success"]:
        print(f"\n[OK] Task completed successfully in {result['elapsed_seconds']:.2f}s ({result['turns']} turn(s))")
        sys.exit(0)
    else:
        print(f"\n[ERROR] Task failed after {result['turns']} turn(s)")
        sys.exit(1)


if __name__ == "__main__":
    main()
