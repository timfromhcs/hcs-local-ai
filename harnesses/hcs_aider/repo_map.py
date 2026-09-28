"""Repository Tag Map Generator for HCS Aider.
Generates an efficient, high-density symbol index of classes, functions, and methods
to fit inside compact context budgets without wasting tokens on full file contents.
"""

import os
import re
from pathlib import Path


class RepoMap:
    def __init__(self, root_dir: str | Path, max_map_tokens: int = 1024):
        self.root_dir = Path(root_dir).resolve()
        self.max_map_tokens = max_map_tokens
        self.ignore_dirs = {
            ".git", "__pycache__", "target", "node_modules", "dist", "build",
            ".pytest_cache", ".venv", "venv", "env", "backup", "artifacts"
        }
        self.supported_exts = {".py", ".rs", ".js", ".ts", ".go", ".c", ".cpp", ".h"}

    def generate_map(self) -> str:
        """Scan repository and produce a concise symbol map."""
        lines = []
        for root, dirs, files in os.walk(self.root_dir):
            dirs[:] = [d for d in dirs if d not in self.ignore_dirs and not d.startswith(".")]
            for file in sorted(files):
                ext = os.path.splitext(file)[1].lower()
                if ext in self.supported_exts:
                    rel_path = (Path(root) / file).relative_to(self.root_dir).as_posix()
                    symbols = self._extract_symbols(Path(root) / file, ext)
                    if symbols:
                        lines.append(f"{rel_path}:")
                        for sym in symbols:
                            lines.append(f"  {sym}")

        full_map = "\n".join(lines)
        # Cap map to token budget (~3.8 chars per token)
        char_limit = self.max_map_tokens * 4
        if len(full_map) > char_limit:
            full_map = full_map[:char_limit] + "\n  [... repo map truncated for context budget ...]"
        return full_map

    def _extract_symbols(self, file_path: Path, ext: str) -> list[str]:
        symbols = []
        try:
            content = file_path.read_text(encoding="utf-8", errors="ignore")
        except Exception:
            return []

        if ext == ".py":
            for line in content.splitlines():
                line_str = line.strip()
                if line_str.startswith("class ") and ":" in line_str:
                    symbols.append(line_str.split(":")[0])
                elif line_str.startswith("def ") and "(" in line_str:
                    symbols.append(line_str.split(":")[0])
                elif line_str.startswith("async def ") and "(" in line_str:
                    symbols.append(line_str.split(":")[0])
        elif ext == ".rs":
            for line in content.splitlines():
                line_str = line.strip()
                if line_str.startswith("pub struct ") or line_str.startswith("struct "):
                    symbols.append(line_str.split("{")[0].strip())
                elif line_str.startswith("pub enum ") or line_str.startswith("enum "):
                    symbols.append(line_str.split("{")[0].strip())
                elif line_str.startswith("pub fn ") or line_str.startswith("fn "):
                    symbols.append(line_str.split("{")[0].strip())
                elif line_str.startswith("pub async fn ") or line_str.startswith("async fn "):
                    symbols.append(line_str.split("{")[0].strip())
        elif ext in {".js", ".ts"}:
            for line in content.splitlines():
                line_str = line.strip()
                if line_str.startswith("class ") or line_str.startswith("export class "):
                    symbols.append(line_str.split("{")[0].strip())
                elif "function " in line_str:
                    symbols.append(line_str.split("{")[0].strip())
        return symbols[:30]  # Cap symbols per file
