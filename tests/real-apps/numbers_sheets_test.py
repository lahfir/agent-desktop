"""Run with python3 tests/real-apps/numbers_sheets_test.py."""
import os
from pathlib import Path
import subprocess
import tempfile


def check_failure_stops_mutations():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        fake = root / "fake-ad"
        calls = root / "calls"
        fake.write_text("""#!/bin/bash
printf '%s\\n' "$*" >> "$CALLS"
if [[ "$1" == click ]]; then
    echo '{"ok":false,"error":{"disposition":{"retry":"unsafe"}}}'
    exit 1
fi
echo '{"ok":true}'
""")
        fake.chmod(0o700)
        result = subprocess.run(
            ["bash", str(Path(__file__).with_name("numbers-sheets.sh")),
             str(fake), "w-123", "@stest:e1", "@stest:e2", "2"],
            env={**os.environ, "CALLS": str(calls), "AGENT_DESKTOP_HOME": str(root)},
            capture_output=True, text=True,
        )
        commands = calls.read_text().splitlines()
        assert result.returncode != 0
        assert commands[0] == "list-windows --app Numbers"
        assert commands[1] == "click @stest:e1"
        assert len(commands) == 3
        assert commands[2].startswith("screenshot --window-id w-123 ")
        assert '"retry":"unsafe"' in result.stdout


if __name__ == "__main__":
    check_failure_stops_mutations()
