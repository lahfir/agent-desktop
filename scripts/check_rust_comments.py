#!/usr/bin/env python3

import sys
from pathlib import Path


def raw_string_end(source, start):
    prefix_length = 2 if source.startswith("br", start) else 1
    if source[start : start + prefix_length] not in ("r", "br"):
        return None
    if start > 0 and (source[start - 1].isalnum() or source[start - 1] == "_"):
        return None
    cursor = start + prefix_length
    hashes = 0
    while cursor < len(source) and source[cursor] == "#":
        hashes += 1
        cursor += 1
    if cursor >= len(source) or source[cursor] != '"':
        return None
    marker = '"' + "#" * hashes
    end = source.find(marker, cursor + 1)
    return len(source) if end < 0 else end + len(marker)


def quoted_string_end(source, start):
    prefix_length = 2 if source.startswith('b"', start) else 1
    if source[start : start + prefix_length] not in ('"', 'b"'):
        return None
    if prefix_length == 2 and start > 0 and (
        source[start - 1].isalnum() or source[start - 1] == "_"
    ):
        return None
    cursor = start + prefix_length
    escaped = False
    while cursor < len(source):
        character = source[cursor]
        if escaped:
            escaped = False
        elif character == "\\":
            escaped = True
        elif character == '"':
            return cursor + 1
        cursor += 1
    return len(source)


def block_comment_end(source, start):
    cursor = start + 2
    depth = 1
    while cursor < len(source) and depth:
        if source.startswith("/*", cursor):
            depth += 1
            cursor += 2
        elif source.startswith("*/", cursor):
            depth -= 1
            cursor += 2
        else:
            cursor += 1
    return cursor


def forbidden_comments(source):
    findings = []
    cursor = 0
    line = 1
    code_on_line = False
    while cursor < len(source):
        raw_end = raw_string_end(source, cursor)
        if raw_end is not None:
            segment = source[cursor:raw_end]
            line += segment.count("\n")
            if "\n" in segment:
                code_on_line = bool(segment.rsplit("\n", 1)[-1].strip())
            else:
                code_on_line = True
            cursor = raw_end
            continue
        string_end = quoted_string_end(source, cursor)
        if string_end is not None:
            segment = source[cursor:string_end]
            line += segment.count("\n")
            if "\n" in segment:
                code_on_line = bool(segment.rsplit("\n", 1)[-1].strip())
            else:
                code_on_line = True
            cursor = string_end
            continue
        if source.startswith("//", cursor):
            is_doc = source.startswith("///", cursor) or source.startswith("//!", cursor)
            if not is_doc:
                kind = "end-of-line comment" if code_on_line else "non-doc line comment"
                findings.append((line, kind))
            end = source.find("\n", cursor + 2)
            if end < 0:
                break
            cursor = end
            continue
        if source.startswith("/*", cursor):
            is_doc = source.startswith("/**", cursor) or source.startswith("/*!", cursor)
            if not is_doc:
                findings.append((line, "block comment"))
            end = block_comment_end(source, cursor)
            line += source[cursor:end].count("\n")
            code_on_line = False if "\n" in source[cursor:end] else code_on_line
            cursor = end
            continue
        character = source[cursor]
        if character == "\n":
            line += 1
            code_on_line = False
        elif not character.isspace():
            code_on_line = True
        cursor += 1
    return findings


def check_path(path):
    if not path.is_file():
        return []
    source = path.read_text(encoding="utf-8")
    if "@generated" in "\n".join(source.splitlines()[:5]):
        return []
    return forbidden_comments(source)


def main():
    failed = False
    for raw_path in sys.stdin.buffer.read().split(b"\0"):
        if not raw_path:
            continue
        path = Path(raw_path.decode())
        for line, kind in check_path(path):
            print(f"{path}:{line}: {kind}s are forbidden", file=sys.stderr)
            failed = True
    raise SystemExit(1 if failed else 0)


if __name__ == "__main__":
    main()
