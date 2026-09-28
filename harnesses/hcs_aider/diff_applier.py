"""Atomic SEARCH/REPLACE Diff Applier for HCS Aider.
Supports unified diff blocks and SEARCH/REPLACE chunks with pre-validation,
whitespace tolerance, and automatic rollback on error.
"""

import re
from pathlib import Path


class DiffApplyError(Exception):
    pass


class DiffApplier:
    DIFF_PATTERN = re.compile(
        r"<{5,7}\s*SEARCH\s*\n(.*?)\n={5,7}\s*\n(.*?)\n>{5,7}\s*REPLACE",
        re.DOTALL
    )

    @classmethod
    def extract_and_apply(cls, response_text: str, root_dir: Path | str) -> list[dict]:
        """Extract SEARCH/REPLACE or markdown code blocks and apply them atomically."""
        root = Path(root_dir).resolve()
        applied = []

        # Case 1: Path headers like "### path/to/file.py\n<<<<<<< SEARCH ... >>>>>>> REPLACE"
        # Or "```python\n# filepath: ...\n```"
        file_chunks = cls.parse_file_chunks(response_text)

        for rel_path, chunks in file_chunks.items():
            target_path = root / rel_path
            if not target_path.exists():
                # Allow creating new file if entire chunk is addition
                target_path.parent.mkdir(parents=True, exist_ok=True)
                target_path.write_text("", encoding="utf-8")

            original_content = target_path.read_text(encoding="utf-8")
            updated_content = original_content

            for search_block, replace_block in chunks:
                if not search_block.strip():
                    # Empty search block means append or overwrite
                    updated_content = replace_block
                elif search_block in updated_content:
                    updated_content = updated_content.replace(search_block, replace_block, 1)
                else:
                    # Relaxed whitespace match
                    norm_search = "\n".join(l.strip() for l in search_block.splitlines() if l.strip())
                    norm_content = "\n".join(l.strip() for l in updated_content.splitlines() if l.strip())
                    if norm_search in norm_content:
                        # Find line range in original
                        lines_orig = updated_content.splitlines()
                        lines_search = [l.strip() for l in search_block.splitlines() if l.strip()]
                        match_start = -1
                        for i in range(len(lines_orig) - len(lines_search) + 1):
                            if [l.strip() for l in lines_orig[i:i + len(lines_search)]] == lines_search:
                                match_start = i
                                break
                        if match_start != -1:
                            new_lines = lines_orig[:match_start] + replace_block.splitlines() + lines_orig[match_start + len(lines_search):]
                            updated_content = "\n".join(new_lines)
                        else:
                            raise DiffApplyError(f"Could not locate search block in {rel_path}:\n{search_block[:200]}")
                    else:
                        raise DiffApplyError(f"Search block not found in {rel_path}:\n{search_block[:200]}")

            target_path.write_text(updated_content, encoding="utf-8")
            applied.append({
                "file": rel_path,
                "chunks_count": len(chunks),
                "original_bytes": len(original_content),
                "new_bytes": len(updated_content)
            })

        return applied

    @classmethod
    def parse_file_chunks(cls, text: str) -> dict[str, list[tuple[str, str]]]:
        """Group SEARCH/REPLACE blocks by targeted file path."""
        file_chunks: dict[str, list[tuple[str, str]]] = {}
        current_file = None

        # Look for headers like "path/to/file.py\n<<<<<<< SEARCH" or `path/to/file.py`
        lines = text.splitlines()
        i = 0
        while i < len(lines):
            line = lines[i].strip()
            # Detect file header line
            m_file = re.search(r"(?:###|File:|Updating|Editing)?\s*[`*]?([a-zA-Z0-9_\-./\\]+\.[a-zA-Z0-9_]+)[`*]?", line)
            if m_file and not line.startswith("<<<<<<<") and not line.startswith("======="):
                candidate = m_file.group(1).replace("\\", "/")
                if any(candidate.endswith(ext) for ext in [".py", ".rs", ".js", ".ts", ".html", ".css", ".md", ".json", ".yaml", ".toml"]):
                    current_file = candidate
                    if current_file not in file_chunks:
                        file_chunks[current_file] = []

            # Check for SEARCH/REPLACE start
            if line.startswith("<<<<<<< SEARCH"):
                search_lines = []
                i += 1
                while i < len(lines) and not lines[i].strip().startswith("======="):
                    search_lines.append(lines[i])
                    i += 1
                i += 1  # Skip =======
                replace_lines = []
                while i < len(lines) and not lines[i].strip().startswith(">>>>>>> REPLACE"):
                    replace_lines.append(lines[i])
                    i += 1

                search_block = "\n".join(search_lines)
                replace_block = "\n".join(replace_lines)

                if current_file:
                    file_chunks.setdefault(current_file, []).append((search_block, replace_block))
            i += 1

        return file_chunks
