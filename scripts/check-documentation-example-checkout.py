#!/usr/bin/env python3
"""Check the actual workflow's checkout repository outputs without network I/O."""
import contextlib
import copy
import io
import json
from pathlib import Path
import re
import textwrap
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
workflow = (ROOT / '.github/workflows/documentation-examples.yml').read_text()
blocks = re.findall(r"python3 - <<'PY' >> \"\$GITHUB_OUTPUT\"\n(.*?)^          PY$", workflow, re.M | re.S)
assert len(blocks) == 1, 'expected exactly one dependency-output Python heredoc'
code = compile(textwrap.dedent(blocks[0]), '<workflow-dependency-output>', 'exec')
data = json.loads((ROOT / 'scripts/documentation-example-dependencies.json').read_text())


def execute(payload):
    output = io.StringIO()
    with patch.object(Path, 'read_text', return_value=json.dumps(payload)):
        with contextlib.redirect_stdout(output):
            exec(code, {'__name__': '__main__'})
    lines = output.getvalue().splitlines()
    pairs = [line.split('=', 1) for line in lines]
    assert all(len(pair) == 2 for pair in pairs), 'malformed output'
    result = dict(pairs)
    assert len(result) == len(lines), 'duplicate output keys'
    return result


expected = {}
for dep in data['dependencies']:
    name = dep['name']
    expected[f'{name}_repository'] = f'qubit-ltd/{name}'
    expected[f'{name}_rev'] = dep['rev']
assert execute(data) == expected, 'checkout outputs must use owner/repo and retain revisions'
for field, bad_value in [('repository', 'https://github.com/other/repo.git'), ('rev', 'not-a-sha')]:
    invalid = copy.deepcopy(data)
    invalid['dependencies'][0][field] = bad_value
    try:
        execute(invalid)
    except SystemExit:
        pass
    else:
        raise AssertionError(f'invalid {field} must be rejected')
print('Checkout dependency outputs and rejection checks passed.')
