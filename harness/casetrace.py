"""The lines of a generator each of its cases runs, for tools/regen_check.py's
per-case cache (tools/casecache.py: a case's key covers the code it ran,
not all the code it could reach).

Imported first by a PER_CASE generator. With ARENA_TRACE=PATH in the
environment it records, through sys.monitoring, which of the generator's
own lines run in each context, and writes them to PATH as JSON on exit;
without it, it does nothing. The contexts:

- `mod:L` while the module's top level runs line L (a CASES entry, a
  table, a constant), with what that line calls;
- `''` from `begin('')`: `main`'s own setup, every case's;
- whatever label `begin` names after that (`place:WHERE`, `case:NAME`),
  with the labels it `uses` (a case uses the place it plays in, built
  under the first case there).

Each line location reports once per context and is then disabled until
the next context begins, so the cost is a few events per line, not one
per line run.
"""
import json
import os
import sys
import time

_out = os.environ.get('ARENA_TRACE')
_gen = os.path.abspath(sys.argv[0]) if _out else None
_ctx = 'mod:0'
_lines = {}
_uses = {}
_secs = {}
_t0 = time.time()
_mod_code = None


def _switch(label):
    global _ctx, _t0
    now = time.time()
    _secs[_ctx] = _secs.get(_ctx, 0.0) + now - _t0
    _t0 = now
    _ctx = label
    sys.monitoring.restart_events()


def _line(code, line):
    global _mod_code
    if code.co_filename != _gen and os.path.abspath(code.co_filename) != _gen:
        return sys.monitoring.DISABLE
    if code.co_name == '<module>' and _ctx.startswith('mod:'):
        _mod_code = code
        if _ctx != f'mod:{line}':
            _switch(f'mod:{line}')
    _lines.setdefault(_ctx, set()).add(line)
    return sys.monitoring.DISABLE


def begin(label, uses=()):
    """Lines from here on are LABEL's (and a case's key covers USES's too)."""
    if not _out:
        return
    _switch(label)
    _uses.setdefault(label, set()).update(uses)


def _write():
    _switch('')
    data = {ctx: {'lines': sorted(lines), 'uses': sorted(_uses.get(ctx, ())), 'secs': round(_secs.get(ctx, 0.0), 3)}
            for ctx, lines in _lines.items()}
    for ctx in _secs:
        data.setdefault(ctx, {'lines': [], 'uses': sorted(_uses.get(ctx, ())), 'secs': round(_secs[ctx], 3)})
    with open(_out, 'w') as f:
        json.dump(data, f)


if _out:
    import atexit
    _tool = next(i for i in range(6) if sys.monitoring.get_tool(i) is None and i not in (0, 1, 2))
    sys.monitoring.use_tool_id(_tool, 'casetrace')
    sys.monitoring.register_callback(_tool, sys.monitoring.events.LINE, _line)
    sys.monitoring.set_events(_tool, sys.monitoring.events.LINE)
    atexit.register(_write)
