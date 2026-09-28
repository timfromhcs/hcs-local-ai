"""Interactive Terminal UI and Universal CLI for HCS Aider.
Supports slash commands, real-time SSE streaming, smart model routing,
Tree-sitter repo maps, atomic Git rollbacks, auto-booting daemon, and telemetry.
"""

import os
import sys
import time
import subprocess
from pathlib import Path
from typing import Optional

import requests
from prompt_toolkit import PromptSession
from prompt_toolkit.completion import Completer, Completion
from prompt_toolkit.styles import Style
from rich.console import Console
from rich.panel import Panel
from rich.markdown import Markdown
from rich.table import Table
from rich.syntax import Syntax
from rich.text import Text

if sys.stdout and hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if sys.stderr and hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

from .agent import HCSAiderAgent
from .diff_applier import DiffApplier, DiffApplyError

console = Console(force_terminal=True, legacy_windows=False)


class RepoFileCompleter(Completer):
    """Auto-complete relative file paths within target repository."""
    def __init__(self, repo_dir: Path):
        self.repo_dir = repo_dir

    def get_completions(self, document, complete_event):
        text_before_cursor = document.text_before_cursor
        word = text_before_cursor.split()[-1] if text_before_cursor.split() else ""

        if word.startswith("--"):
            return

        clean_word = word.lstrip("/")
        try:
            for root, dirs, files in os.walk(self.repo_dir):
                dirs[:] = [d for d in dirs if not d.startswith(".") and d not in ("target", "venv", "__pycache__", "node_modules", "dist", "build")]
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

    with console.status("[bold cyan]HCS Daemon offline. Auto-booting background service...[/bold cyan]", spinner="dots"):
        candidate_dirs = [
            Path.cwd(),
            Path("E:/AI Serving Stack"),
            Path(__file__).resolve().parent.parent.parent,
        ]
        daemon_exe = None
        for cd in candidate_dirs:
            p = cd / "hcs-daemon.exe"
            if p.exists():
                daemon_exe = p
                break

        if not daemon_exe:
            console.print("[bold red]hcs-daemon.exe not found. Please start daemon via start.bat.[/bold red]")
            return False

        try:
            subprocess.Popen(
                [str(daemon_exe), "run"],
                cwd=str(daemon_exe.parent),
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0,
            )
            for _ in range(25):
                time.sleep(1)
                try:
                    r = requests.get(f"{base_url}/hcs/v1/system", timeout=1)
                    if r.status_code == 200:
                        console.print("[bold green]✔ HCS Daemon auto-booted successfully![/bold green]")
                        return True
                except Exception:
                    pass
        except Exception as e:
            console.print(f"[red]Failed to spawn daemon: {e}[/red]")
            return False

    console.print("[red]Daemon failed to report healthy within 25 seconds.[/red]")
    return False


class InteractiveHCSAider:
    def __init__(self, repo_dir: Path, enable_thinking: bool = False, model: str = "auto"):
        self.repo_dir = repo_dir.resolve()
        self.enable_thinking = enable_thinking
        self.focused_files: list[str] = []
        self.current_model = model
        self.agent = HCSAiderAgent(
            repo_dir=self.repo_dir,
            enable_thinking=self.enable_thinking,
            model=self.current_model,
            max_tokens=4096,
        )
        try:
            self.session = PromptSession(
                completer=RepoFileCompleter(self.repo_dir),
                style=Style.from_dict({
                    "prompt": "#00ffaf bold",
                    "slash": "#ffb000 bold",
                }),
            )
        except Exception:
            self.session = None

    def print_banner(self):
        banner_text = f"""[bold cyan]HCS AIDER[/bold cyan] [white]— Autonomous Software Engineering & Local Pair Programmer[/white]
[dim]Repository :[/dim] [yellow]{self.repo_dir}[/yellow]
[dim]Model Mode :[/dim] [green]{self.current_model}[/green] [dim](Smart dispatch: hcs-coder 27B / hcs-general 4B / hcs-judge 4B)[/dim]
[dim]KV Cache   :[/dim] [cyan]Unified Q4_0 with Flash-Attention FP32 Accumulator[/cyan]
[dim]Commands   :[/dim] [bold]/add, /drop, /ls, /model, /status, /map, /diff, /undo, /test, /think, /help, /exit[/bold]"""
        console.print(Panel(banner_text, border_style="cyan", title="⚡ [bold]HCS Local AI v5.0.3 Stable[/bold]"))

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
            console.print(f"[green]✔ Added {len(added)} file(s) to active focus: {', '.join(added)}[/green]")

    def cmd_drop(self, files: list[str]):
        for f in files:
            if f in self.focused_files:
                self.focused_files.remove(f)
                console.print(f"[yellow]Dropped {f} from active focus[/yellow]")

    def cmd_ls(self):
        if not self.focused_files:
            console.print("[dim]No files currently in active focus. Use /add <file> or query directly.[/dim]")
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
        with console.status("[dim]Generating Tree-sitter AST symbol map...[/dim]", spinner="dots"):
            map_text = self.agent.repo_map.generate_map()
        console.print(Panel(map_text, title="Repository AST Symbol Map", border_style="blue"))

    def cmd_diff(self):
        try:
            res = subprocess.run(["git", "diff"], cwd=str(self.repo_dir), capture_output=True, text=True)
            diff = res.stdout
            if diff.strip():
                syntax = Syntax(diff, "diff", theme="monokai", line_numbers=False)
                console.print(Panel(syntax, title="Current Uncommitted Git Diff", border_style="yellow"))
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
        with console.status(f"[bold yellow]Running test suite ({test_cmd or 'auto-detected'})...[/bold yellow]", spinner="aesthetic"):
            passed, out = self.agent.run_tests(test_cmd)
        if passed:
            console.print("[bold green]✔ All Tests PASSED successfully![/bold green]")
        else:
            console.print("[bold red]❌ Tests FAILED:[/bold red]")
            console.print(Panel(out[-1200:], border_style="red", title="Test Failure Trace"))

    def cmd_model(self, args: list[str]):
        if not args:
            console.print(f"[cyan]Current model:[/cyan] [bold green]{self.current_model}[/bold green]")
            console.print("[dim]Usage: /model [auto | hcs-coder | hcs-general | hcs-judge | hcs-subagent | hcs-vlm][/dim]")
            return
        m = args[0].strip().lower()
        valid = {
            "auto": "auto",
            "coder": "hcs-coder",
            "hcs-coder": "hcs-coder",
            "general": "hcs-general",
            "hcs-general": "hcs-general",
            "judge": "hcs-judge",
            "hcs-judge": "hcs-judge",
            "subagent": "hcs-subagent",
            "hcs-subagent": "hcs-subagent",
            "vlm": "hcs-vlm",
            "hcs-vlm": "hcs-vlm",
        }
        if m in valid:
            self.current_model = valid[m]
            self.agent.model = self.current_model
            console.print(f"[bold green]✔ Model switched to: {self.current_model}[/bold green]")
        else:
            console.print(f"[red]Unknown model '{m}'. Choose from: auto, hcs-coder, hcs-general, hcs-judge, hcs-subagent, hcs-vlm[/red]")

    def cmd_status(self):
        try:
            r = requests.get(f"{self.agent.hcs_api_url.replace('/v2', '/v1')}/system", timeout=2)
            if r.status_code == 200:
                data = r.json()
                table = Table(title="HCS Local AI System Status", border_style="cyan")
                table.add_column("Property", style="dim")
                table.add_column("Value", style="bold green")
                table.add_row("Daemon Version", f"v{data.get('version', '5.0.3')}")
                table.add_row("Active Model", self.current_model)
                hw = data.get("hardware", {})
                table.add_row("Total RAM", f"{hw.get('total_ram_mb', 'N/A')} MB")
                table.add_row("Available UMA", f"{hw.get('available_ram_mb', 'N/A')} MB")
                table.add_row("Vulkan Platform", str(hw.get("has_vulkan", True)))
                console.print(table)
            else:
                console.print(f"[yellow]Daemon responded with status {r.status_code}[/yellow]")
        except Exception as e:
            console.print(f"[red]Failed to query system status: {e}[/red]")

    def handle_user_prompt(self, user_input: str):
        """Execute query with smart routing, real-time SSE streaming, and diff application."""
        # 1. Smart Model Routing
        target_model, category, reason = self.agent.smart_route_model(user_input, self.focused_files)

        # Routing Info Card
        routing_info = Text.assemble(
            ("🧠 Model Router : ", "dim"),
            (f"{target_model}", "bold green"),
            ("  •  ", "dim"),
            (f"{reason}", "yellow"),
        )
        console.print(Panel(routing_info, border_style="dim", style="dim"))

        # 2. Setup Streaming State & Loading Spinner
        spinner = console.status(
            f"[bold cyan]Warming up {target_model} on AMD Vulkan & processing prompt...[/bold cyan]",
            spinner="dots12",
        )
        spinner.start()

        def on_stream_start():
            if spinner:
                spinner.stop()
            console.print(f"[bold cyan]⚡ {target_model} ❯ [/bold cyan]", end="")

        def on_stream_token(token: str):
            sys.stdout.write(token)
            sys.stdout.flush()

        try:
            res = self.agent.query_model_stream(
                user_prompt=user_input,
                target_files=self.focused_files,
                on_start=on_stream_start,
                on_token=on_stream_token,
                model_override=target_model,
            )
        finally:
            if spinner:
                spinner.stop()

        print()  # Final newline after token stream

        # 3. Performance Metrics Pill
        ttft = res.get("ttft", 0.0)
        tokens = res.get("total_tokens", 0)
        tps = res.get("tps", 0.0)
        total_time = res.get("duration_sec", 0.0)

        metrics_text = Text.assemble(
            ("⚡ Inference : ", "dim"),
            (f"{total_time:.2f}s total", "cyan"),
            ("  │  TTFT: ", "dim"),
            (f"{ttft:.2f}s", "green"),
            ("  │  Tokens: ", "dim"),
            (f"{tokens}", "yellow"),
            ("  │  Speed: ", "dim"),
            (f"{tps:.1f} tok/s", "bold cyan"),
            ("  │  Model: ", "dim"),
            (f"{target_model}", "magenta"),
        )
        console.print(Panel(metrics_text, border_style="dim"))

        # 4. Check for code modification diffs
        response_text = res.get("content", "")
        file_chunks = DiffApplier.parse_file_chunks(response_text)
        if file_chunks:
            console.print(f"\n[bold yellow]🔍 Detected {len(file_chunks)} file modification block(s):[/bold yellow]")
            for fpath in file_chunks.keys():
                console.print(f"  • [cyan]{fpath}[/cyan]")

            try:
                applied = DiffApplier.extract_and_apply(response_text, self.repo_dir)
                console.print(f"[bold green]✔ Successfully applied changes to {len(applied)} file(s)![/bold green]")
                # Optionally offer to run tests if tests are present
                if (self.repo_dir / "pytest.ini").exists() or (self.repo_dir / "tests").exists() or (self.repo_dir / "Cargo.toml").exists():
                    self.cmd_test([])
            except DiffApplyError as e:
                console.print(f"[bold red]❌ Diff application error: {e}[/bold red]")

    def run_interactive(self):
        self.print_banner()

        while True:
            try:
                prompt_label = f"hcsaider [{self.current_model}] ❯ "
                if self.session:
                    user_input = self.session.prompt(prompt_label).strip()
                else:
                    user_input = input(f"hcsaider [{self.current_model}] > ").strip()
                if not user_input:
                    continue

                if user_input in ("/exit", "/quit"):
                    console.print("[dim]Exiting HCS Aider. Goodbye![/dim]")
                    break

                if user_input == "/clear":
                    os.system("cls" if os.name == "nt" else "clear")
                    self.print_banner()
                    continue

                if user_input.startswith("/"):
                    parts = user_input.split()
                    cmd = parts[0].lower()
                    args = parts[1:]

                    if cmd == "/help":
                        console.print(
                            "\n[bold cyan]HCS Aider Commands:[/bold cyan]\n"
                            "  [bold green]/model [name][/bold green]   - View or switch active model (auto, coder, general, judge, vlm)\n"
                            "  [bold green]/status[/bold green]         - View live daemon status, RAM, and Vulkan hardware profile\n"
                            "  [bold green]/add <files>[/bold green]    - Add specific files to active context focus\n"
                            "  [bold green]/drop <files>[/bold green]   - Remove files from focus\n"
                            "  [bold green]/ls[/bold green]            - List currently focused files\n"
                            "  [bold green]/map[/bold green]           - Display Tree-sitter abstract syntax tree map\n"
                            "  [bold green]/diff[/bold green]          - Show current uncommitted Git diff with syntax highlighting\n"
                            "  [bold green]/undo[/bold green]          - Rollback all uncommitted changes to Git HEAD\n"
                            "  [bold green]/test [cmd][/bold green]    - Execute repository test suite\n"
                            "  [bold green]/think [on|off][/bold green]- Toggle deep reasoning tokens\n"
                            "  [bold green]/clear[/bold green]         - Clear terminal screen and redraw banner\n"
                            "  [bold green]/exit[/bold green]          - Quit interactive session\n"
                        )
                    elif cmd == "/model":
                        self.cmd_model(args)
                    elif cmd == "/status" or cmd == "/hw":
                        self.cmd_status()
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

                # Execute natural language prompt with streaming
                self.handle_user_prompt(user_input)

            except (KeyboardInterrupt, EOFError):
                console.print("\n[dim]Session closed.[/dim]")
                break
            except Exception as e:
                console.print(f"[bold red]Unexpected error: {e}[/bold red]")
