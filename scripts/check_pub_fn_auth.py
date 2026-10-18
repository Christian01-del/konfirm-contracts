#!/usr/bin/env python3
"""Fail if any `pub fn` in contracts/*/src/lib.rs neither calls
require_auth / require_signer / require_admin in its body nor carries a
`/// Permissionless` doc comment. See "Authorization convention" in README.md.
"""

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PUB_FN = re.compile(r"^\s*pub fn (\w+)")
AUTH_CALL = re.compile(r"\brequire_(auth|signer|admin)\b")
PERMISSIONLESS = re.compile(r"^\s*///\s*Permissionless\b")


def is_permissionless(lines, index):
    i = index - 1
    while i >= 0 and lines[i].lstrip().startswith(("///", "#[")):
        if PERMISSIONLESS.match(lines[i]):
            return True
        i -= 1
    return False


def fn_body(lines, start):
    code = "\n".join(line.split("//", 1)[0] for line in lines[start:])
    open_at = code.index("{")
    depth = 0
    for i in range(open_at, len(code)):
        if code[i] == "{":
            depth += 1
        elif code[i] == "}":
            depth -= 1
            if depth == 0:
                return code[open_at:i]
    raise ValueError(f"unclosed body for fn at line {start + 1}")


def main():
    found = 0
    failures = []
    for path in sorted(ROOT.glob("contracts/*/src/lib.rs")):
        lines = path.read_text().splitlines()
        for index, line in enumerate(lines):
            match = PUB_FN.match(line)
            if not match:
                continue
            found += 1
            if is_permissionless(lines, index) or AUTH_CALL.search(fn_body(lines, index)):
                continue
            failures.append(
                f"{path.relative_to(ROOT)}:{index + 1}: pub fn {match.group(1)} calls no "
                "require_auth/require_signer/require_admin and is not documented `/// Permissionless`"
            )

    if found == 0:
        print("no pub fns found under contracts/*/src/lib.rs; check the glob", file=sys.stderr)
        return 1
    for failure in failures:
        print(failure, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
