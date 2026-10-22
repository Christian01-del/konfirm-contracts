#!/usr/bin/env python3
"""Fail if any `pub fn` in contracts/*/src/lib.rs neither calls
require_auth / require_signer / require_admin in its body nor carries a
`/// Permissionless` doc comment. See "Authorization convention" in README.md.

Also verifies (issue #95) that secret scanning and push protection are
enabled on this public fund-custody repo. GitHub repo settings cannot be
toggled from code; an admin must enable them via Settings -> Code security
or the API. This check fails loudly if they are still disabled.

Verify manually with:
    gh api repos/Konfirm-xyz/konfirm-contracts --jq '.security_and_analysis'

Required (acceptance criteria for #95):
    secret_scanning.status == "enabled"
    secret_scanning_push_protection.status == "enabled"

Recommended rollout: enable secret scanning first, then push protection,
allow-listing any legitimate testnet/test fixture patterns that trigger
false positives.
"""

import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PUB_FN = re.compile(r"^\s*pub fn (\w+)")
AUTH_CALL = re.compile(r"\brequire_(auth|signer|admin)\b")
PERMISSIONLESS = re.compile(r"^\s*///\s*Permissionless\b")

REPO = "Konfirm-xyz/konfirm-contracts"
REQUIRED_SECURITY = ("secret_scanning", "secret_scanning_push_protection")


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


def check_security_settings():
    """Return a list of failures for disabled secret scanning settings."""
    try:
        result = subprocess.run(
            ["gh", "api", f"repos/{REPO}", "--jq", ".security_and_analysis"],
            capture_output=True,
            text=True,
            check=True,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        return [
            f"could not query security_and_analysis for {REPO} ({exc}); "
            "run `gh api repos/Konfirm-xyz/konfirm-contracts --jq .security_and_analysis`"
        ]

    try:
        analysis = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        return [f"could not parse security_and_analysis for {REPO}: {exc}"]

    failures = []
    for feature in REQUIRED_SECURITY:
        status = (analysis.get(feature) or {}).get("status")
        if status != "enabled":
            failures.append(
                f"{REPO}: {feature}.status is {status!r}, expected 'enabled' "
                "(enable via Settings -> Code security or the API)"
            )
    return failures


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
    failures.extend(check_security_settings())
    for failure in failures:
        print(failure, file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
