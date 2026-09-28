"""CLI Entry Point for HCS Aider Autonomous Coding Agent.
Supports interactive terminal mode (default) and batch script mode (--task).
"""

import argparse
import sys
from pathlib import Path
from .agent import HCSAiderAgent
from .cli import InteractiveHCSAider, ensure_daemon_online


def main():
    parser = argparse.ArgumentParser(description="HCS Aider - Native Autonomous Coding Agent")
    parser.add_argument("files", nargs="*", default=[], help="Files to focus on immediately")
    parser.add_argument("--dir", default=".", help="Target repository directory (default: current directory)")
    parser.add_argument("--model", default="auto", help="Inference model: auto, hcs-coder, hcs-general, hcs-judge, hcs-vlm (default: auto)")
    parser.add_argument("--task", default=None, help="Coding task or bugfix instruction (if omitted, launches interactive UI)")
    parser.add_argument("--test-cmd", default=None, help="Custom test command (e.g. 'pytest tests/test_core.py')")
    parser.add_argument("--thinking", action="store_true", help="Enable deep reasoning thinking mode")
    parser.add_argument("--max-tokens", type=int, default=2048, help="Maximum working generation tokens (default: 2048)")

    args = parser.parse_args()
    repo_path = Path(args.dir).resolve()

    # Ensure backend is online (auto-boots if needed)
    ensure_daemon_online()

    if args.task:
        # Batch Execution Mode
        agent = HCSAiderAgent(
            repo_dir=repo_path,
            enable_thinking=args.thinking,
            model=args.model,
            max_tokens=args.max_tokens,
        )
        res = agent.query_model_stream(
            user_prompt=args.task,
            target_files=args.files,
            on_token=lambda tok: (sys.stdout.write(tok), sys.stdout.flush()),
        )
        sys.exit(0)
    else:
        # Interactive Terminal UI Mode
        ui = InteractiveHCSAider(repo_dir=repo_path, enable_thinking=args.thinking, model=args.model)
        if args.files:
            ui.cmd_add(args.files)
        ui.run_interactive()


if __name__ == "__main__":
    main()
