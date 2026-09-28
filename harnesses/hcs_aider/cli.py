"""Interactive Terminal UI and Universal CLI for HCS Aider.
Supports slash commands, Tree-sitter repo maps, atomic Git rollbacks,
auto-booting the local HCS daemon, and live syntax-highlighted streaming.
"""

import os
import sys
import time
import subprocess
import requests
from pathlib import Path
from typing import Optional

from prompt_toolkit import PromptSession
from prompt_toolkit.completion import Completer, Completion
from prompt_toolkit.styles import Style
from rich.console import Console
from rich.panel import Panel
from rich.markdown import Markdown
from rich.table import Table

from .agent import HCSAiderAgent
from .diff_applier import DiffApplier, DiffApplyError

console = Console()


class RepoFileCompleter(Completer):
    """Auto-complete relative file paths within target repository."""
    def __init__(self, repo_dir: Path):
        self.repo_dir = repo_dir

    def get_completions(self, document, complete_event):
        text_before_cursor = document.text_before_cursor
        word = text_before_cursor.split()[-1] if text_before_cursor.split() else ""
        
        # Don't autocomplete options
        if word.startswith("--"):
            return

        clean_word = word.lstrip("/")
        try:
            for root, dirs, files in os.walk(self.repo_dir):
                # Skip .git and caches
                dirs[:] = [d for d in dirs if not d.startswith(".") and d not in ("target", "venv", "__pycache__", "node_modules")]
                rel_root = Path(root).relative_to(self.repo_dir)

                for f in files:
                    rel_path = (rel_root / f).as_posix().lstrip("./")
                    if clean_word.lower() in rel_path.lower():
                        yield Completion(rel_path, start_position=-len(word))
        except Exception:
            pass


def ensure_daemon_online(base_url: str = "http://127.0.0.1:8787") -> bool:
    """Verify local HCS daemon is online, or auto-boot in background."""
    try:
        r = requests.get(f"{base_url}/hcs/v1/system", timeout=2)
        if r.status_code == 200:
            return True
    except Exception:
        pass

    console.print("[yellow]⚡ Local HCS Daemon offline. Attempting auto-boot...[/yellow]")

    # Find daemon executable
    candidates = [
        Path.cwd() / "hcs-daemon.exe",
        Path("E:/AI Serving Stack/hcs-daemon.exe"),
        Path(os.environ.get("LOCALAPPDATA", "")) / "Programs/HCS-Local-AI/hcs-daemon.exe",
        Path.home() / ".local/bin/hcs-daemon.exe",
    ]

    daemon_exe = None
    for cand in candidates:
        if cand.exists():
            daemon_exe = cand
            break

    if not daemon_exe:
        console.print("[red]❌ Could not locate hcs-daemon.exe to auto-start.[/red]")
        console.print("[dim]Please start the server with start.bat or ensure hcs-daemon is installed.[/dim]")
        return False

    console.print(f"[dim]Starting daemon from: {daemon_exe}[/dim]")
    try:
        if sys.platform == "win32":
            DETACHED_PROCESS = 0x00000008
            CREATE_NEW_PROCESS_GROUP = 0x00000200
            subprocess.Popen(
                [str(daemon_exe), "run"],
                creationflags=DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                close_fds=True,
            )
        else:
            subprocess.Popen([str(daemon_exe), "run"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)

        for attempt in range(40):
            time.sleep(0.5)
            try:
                r = requests.get(f"{base_url}/hcs/v1/system", timeout=1)
                if r.status_code == 200:
                    console.print("[green]✔ HCS Daemon is online and healthy![/green]")
                    return True
            except Exception:
                pass
    except Exception as e:
        console.print(f"[red]Failed to spawn daemon: {e}[/red]")
        return False

    console.print("[red]Daemon failed to report healthy within 20 seconds.[/red]")
    return False


class InteractiveHCSAider:
    def __init__(self, repo_dir: Path, enable_thinking: bool = False):
        self.repo_dir = repo_dir.resolve()
        self.enable_thinking = enable_thinking
        self.focused_files: list[str] = []
        self.agent = HCSAiderAgent(
            repo_dir=self.repo_dir,
            enable_thinking=self.enable_thinking,
            max_tokens=4096,
        )
        self.session = PromptSession(
            completer=RepoFileCompleter(self.repo_dir),
            style=Style.from_dict({
                "prompt": "#00d7af bold",
                "slash": "#ffaf00 bold",
            }),
        )

    def print_banner(self):
        banner_text = f"""[bold cyan]HCS AIDER — Autonomous Software Engineering Agent[/bold cyan]
[dim]Repository:[/dim] [yellow]{self.repo_dir}[/yellow]
[dim]Model:[/dim] [green]hcs-coder (Bonsai 2-27B on AMD iGPU Vulkan, 64k Context)[/green]
[dim]KV Cache:[/dim] [cyan]Unified Q4_0 with Flash-Attention (FP32 Accumulator)[/cyan]
[dim]Decider Gate:[/dim] [blue]hcs-judge (OpenJev 4B in J-Space)[/blue] | [dim]Compactor:[/dim] [magenta]hcs-subagent (1.7B)[/magenta]
[dim]Commands:[/dim] [bold]/add, /drop, /ls, /map, /diff, /undo, /test, /think, /help, /exit[/bold]"""
        console.print(Panel(banner_text, border_style="cyan", title="⚡ HCS Local AI v5.0.0 Stable"))

    def cmd_add(self, files: list[str]):
        added = []
        for f in files:
            path = self.repo_dir / f
            if path.exists() and path.is_file():
                rel = path.relative_to(self.repo_dir).as_posix()
                if rel not in self.focused_files:
                    self.focused_files.append(rel)
                    added.append(rel)
            else:
                console.print(f"[red]File not found: {f}[/red]")
        if added:
            console.print(f"[green]Added {len(added)} file(s) to active focus: {', '.join(added)}[/green]")

    def cmd_drop(self, files: list[str]):
        for f in files:
            if f in self.focused_files:
                self.focused_files.remove(f)
                console.print(f"[yellow]Dropped {f} from active focus[/yellow]")

    def cmd_ls(self):
        if not self.focused_files:
            console.print("[dim]No files currently in active focus. Use /add <file> or request edits directly.[/dim]")
            return
        table = Table(title="Active Focused Files", border_style="dim")
        table.add_column("File Path", style="cyan")
        table.add_column("Size", justify="right", style="green")
        for f in self.focused_files:
            p = self.repo_dir / f
            sz = f"{p.stat().st_size:,} bytes" if p.exists() else "Missing"
            table.add_row(f, sz)
        console.print(table)

    def cmd_map(self):
        console.print("[dim]Generating Tree-sitter Abstract Syntax Tree map...[/dim]")
        map_text = self.agent.repo_map.generate_map()
        console.print(Panel(map_text, title="Repository AST Symbol Map", border_style="blue"))

    def cmd_diff(self):
        try:
            res = subprocess.run(["git", "diff"], cwd=str(self.repo_dir), capture_output=True, text=True)
            diff = res.stdout
            if diff.strip():
                console.print(Panel(diff, title="Current Uncommitted Git Diff", border_style="yellow"))
            else:
                console.print("[dim]No uncommitted git changes detected.[/dim]")
        except Exception as e:
            console.print(f"[red]Error querying git: {e}[/red]")

    def cmd_undo(self):
        try:
            subprocess.run(["git", "checkout", "HEAD", "--", "."], cwd=str(self.repo_dir), check=True)
            console.print("[green]✔ Reverted all uncommitted modifications back to HEAD.[/green]")
        except Exception as e:
            console.print(f"[red]Undo failed: {e}[/red]")

    def cmd_test(self, cmd_args: list[str]):
        test_cmd = " ".join(cmd_args) if cmd_args else None
        console.print(f"[dim]Running tests with: {test_cmd or 'auto-detected suite'}...[/dim]")
        passed, out = self.agent.run_tests(test_cmd)
        if passed:
            console.print("[green]✔ Tests PASSED successfully![/green]")
        else:
            console.print("[red]❌ Tests FAILED:[/red]")
            console.print(Panel(out[-1000:], border_style="red", title="Test Failure Trace"))

    def run_interactive(self):
        self.print_banner()

        while True:
            try:
                user_input = self.session.prompt("hcsaider ❯ ").strip()
                if not user_input:
                    continue

                if user_input in ("/exit", "/quit"):
                    console.print("[dim]Exiting HCS Aider. Goodbye![/dim]")
                    break

                if user_input.startswith("/"):
                    parts = user_input.split()
                    cmd = parts[0].lower()
                    args = parts[1:]

                    if cmd == "/help":
                        console.print(
                            "Available Commands:\n"
                            "  /add <files...>   - Add files to active focus\n"
                            "  /drop <files...>  - Remove files from focus\n"
                            "  /ls               - List currently focused files\n"
                            "  /map              - Display Tree-sitter symbol map\n"
                            "  /diff             - View git diff\n"
                            "  /undo             - Revert changes to HEAD\n"
                            "  /test [cmd]       - Run tests\n"
                            "  /think [on|off]   - Toggle deep reasoning\n"
                            "  /compact          - Force context compaction\n"
                            "  /exit             - Exit\n"
                        )
                    elif cmd == "/add":
                        self.cmd_add(args)
                    elif cmd == "/drop":
                        self.cmd_drop(args)
                    elif cmd == "/ls":
                        self.cmd_ls()
                    elif cmd == "/map":
                        self.cmd_map()
                    elif cmd == "/diff":
                        self.cmd_diff()
                    elif cmd == "/undo":
                        self.cmd_undo()
                    elif cmd == "/test":
                        self.cmd_test(args)
                    elif cmd == "/think":
                        if args and args[0].lower() == "off":
                            self.enable_thinking = False
                            self.agent.enable_thinking = False
                            console.print("[dim]Thinking mode: DISABLED[/dim]")
                        else:
                            self.enable_thinking = True
                            self.agent.enable_thinking = True
                            console.print("[green]Thinking mode: ENABLED (Deep reasoning tokens active)[/green]")
                    else:
                        console.print(f"[red]Unknown command '{cmd}'. Type /help for available options.[/red]")
                    continue

                # Execute natural language task with self-healing
                console.print(f"[bold cyan]⚡ Running Task on '{self.repo_dir.name}':[/bold cyan] {user_input}")
                res = self.agent.execute_task_with_self_healing(
                    task_instruction=user_input,
                )

                if res["success"]:
                    console.print(f"[bold green]✔ All tests passing ({res['turns']} turn(s) in {res['elapsed_seconds']:.2f}s)[/bold green]")
                else:
                    console.print(f"[bold red]❌ Task uncompleted after {res['turns']} turn(s). Traceback available in /test.[/bold red]")

            except (KeyboardInterrupt, EOFError):
                console.print("\n[dim]Session closed.[/dim]")
                break
            except Exception as e:
                console.print(f"[red]Error during execution: {e}[/red]")
