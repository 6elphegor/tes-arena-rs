"""Per-case cache keys for a generator built on a CASES table
(tools/regen_check.py's PER_CASE generators), over the code each case ran.

Such a generator writes a header (lines without a case's name) and then,
for each case of CASES in order, a block of lines whose second field
(`kind | NAME | ...`) is the case's name, and runs only the cases named in
ARENA_CASES (comma-separated). It imports harness/casetrace.py first,
which records the generator's lines each case runs (`main`'s setup, the
place it plays in, its own run, and at import the CASES entry and the
constants it reads).

A case's key is over its CASES entry, its entries in the generator's
case-keyed dicts (FUZZ), which case-name tables hold it, the generator's
top-level statements that aren't definitions (imports), and, of each
top-level function whose body it ran, the source as parsed with every
block it never entered cut out (`masked`), and every top-level constant
or class the code kept names. Everything outside the generator (its
harness modules, the EXE, the game, the save) is regen_check's key.
Where the kept code is unchanged the case runs the same lines (the cut
blocks were never entered, and what decides that is kept), so its block
is the same: editing a branch or a vocabulary a case never reached, a
comment, or another case leaves it alone.

The cut blocks are named by path (`body.3.orelse`), kept with the case's
deps from its last run (`deps`), and cut the same way from the current
source to make the key (`key`); a path that no longer leads to a block
means the code moved, and the case runs again.

The kept code's names are followed by name, over the generator's
top-level definitions: a constant reached only through strings (getattr,
globals) isn't seen, so a generator for this cache must not read its
constants that way. A case-keyed dict or case-name table is keyed by the
case's own entry or membership alone, so code must only index or test
them by the case's name, never walk them.
"""
import ast
import copy
import hashlib

MASK = ast.Expr(ast.Constant('<never run>'))
FIELDS = ('body', 'orelse', 'finalbody', 'handlers', 'cases')


def _span(node):
    return range(node.lineno, node.end_lineno + 1)


def _hit(node, lines):
    return any(n in lines for n in _span(node))


def _masks(node, lines, prefix=''):
    """The paths of NODE's statement blocks none of LINES fall in (not
    looking inside them)."""
    out = []
    for field in FIELDS:
        v = getattr(node, field, None)
        if not isinstance(v, list) or not v or not isinstance(v[0], ast.AST):
            continue
        p = prefix + field
        if field in ('handlers', 'cases'):
            for i, h in enumerate(v):
                out += _masks(h, lines, f'{p}.{i}.')
        elif not any(_hit(s, lines) for s in v):
            out.append(p)
        else:
            for i, s in enumerate(v):
                out += _masks(s, lines, f'{p}.{i}.')
    return out


def _masked(node, paths):
    """NODE with the blocks PATHS name cut out, or None if one doesn't
    lead to a block."""
    node = copy.deepcopy(node)
    for path in paths:
        parts = path.split('.')
        at = node
        try:
            for field, index in zip(parts[0::2], parts[1::2]):
                at = getattr(at, field)[int(index)]
            block = getattr(at, parts[-1])
        except (AttributeError, IndexError, ValueError):
            return None
        if not isinstance(block, list) or not block:
            return None
        block[:] = [MASK]
    return node


def _names(node):
    return {n.id for n in ast.walk(node) if isinstance(n, ast.Name)}


class Cases:
    """The CASES table of a generator's source, its other top-level
    statements, and the keys of its cases."""

    def __init__(self, src):
        tree = ast.parse(src)
        self.tops = {}      # top-level name -> its node (def, class, assignment)
        self.preamble = []  # imports and other top-level statements
        self.order = []     # case names, in CASES order
        self.entries = {}   # case name -> its CASES key and value nodes
        self.tables = {}    # case-name table -> the names it holds
        self.dicts = {}     # case-keyed dict -> {case name: (key, value)}
        cases_node = None
        for node in tree.body:
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                self.tops[node.name] = node
            elif isinstance(node, ast.Assign) and all(isinstance(t, ast.Name) for t in node.targets):
                if [t.id for t in node.targets] == ['CASES'] and isinstance(node.value, ast.Dict):
                    cases_node = node.value
                    continue
                for t in node.targets:
                    self.tops[t.id] = node
            else:
                self.preamble.append(node)
        if cases_node is None:
            raise ValueError('no CASES = {...} table')
        for k, v in zip(cases_node.keys, cases_node.values):
            if not (isinstance(k, ast.Constant) and isinstance(k.value, str)):
                raise ValueError('a CASES key that is not a string')
            self.order.append(k.value)
            self.entries[k.value] = (k, v)
        case_set = set(self.order)
        for n, node in list(self.tops.items()):
            value = getattr(node, 'value', None)
            if isinstance(value, (ast.Tuple, ast.List, ast.Set)) and value.elts and all(
                    isinstance(e, ast.Constant) and e.value in case_set for e in value.elts):
                self.tables[n] = {e.value for e in value.elts}
                del self.tops[n]
            elif isinstance(value, ast.Dict) and value.keys and all(
                    isinstance(k, ast.Constant) and k.value in case_set for k in value.keys):
                self.dicts[n] = {k.value: (k, v) for k, v in zip(value.keys, value.values)}
                del self.tops[n]

    def _own(self, name):
        """The nodes of NAME's own entries (CASES and the case-keyed dicts)."""
        own = list(self.entries[name]) if name is not None else []
        for d in sorted(self.dicts):
            own += list(self.dicts[d].get(name, ()))
        return own

    def deps(self, name, ran, mod):
        """What case NAME's key covers (None: the header), from the lines
        RAN (its contexts but the module's) and MOD ({line: lines} that the
        module's line ran at import): {'defs': {function: [cut paths]},
        'consts': [names]}."""
        lines = set(ran)
        for node in self._own(name):
            for n in _span(node):
                lines |= mod.get(n, set())
        consts = set()
        while True:
            defs = {}
            for n, node in self.tops.items():
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and any(
                        _hit(s, lines) for s in node.body):
                    defs[n] = _masks(node, lines)
            kept = set()
            for n, paths in defs.items():
                kept |= _names(_masked(self.tops[n], paths))
            for node in self._own(name) + self.preamble + [self.tops[c] for c in consts]:
                kept |= _names(node)
            # (A function named but never run doesn't matter: only what it
            # would run, and that is in `defs` once it does.)
            new = {n for n in kept if n in self.tops and n not in consts
                   and not isinstance(self.tops[n], (ast.FunctionDef, ast.AsyncFunctionDef))}
            if not new:
                return {'defs': defs, 'consts': sorted(consts)}
            consts |= new
            for c in new:
                for n in _span(self.tops[c]):
                    lines |= mod.get(n, set())

    def key(self, name, deps):
        """Case NAME's key under DEPS (`deps`), from the current source; None
        when a function or constant it covers is gone or a cut block moved."""
        h = hashlib.sha256()
        for node in self.preamble:
            h.update(b'pre\0' + ast.unparse(node).encode() + b'\0')
        for node in self._own(name):
            h.update(b'own\0' + ast.unparse(node).encode() + b'\0')
        for d in sorted(self.dicts):
            h.update(b'dict\0' + d.encode() + (b'\0has' if name in self.dicts[d] else b'\0not') + b'\0')
        for t in sorted(self.tables):
            h.update(b'table\0' + t.encode() + (b'\0in' if name in self.tables[t] else b'\0out') + b'\0')
        for n in sorted(deps['defs']):
            node = self.tops.get(n)
            if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                return None
            masked = _masked(node, deps['defs'][n])
            if masked is None:
                return None
            h.update(b'def\0' + ast.unparse(masked).encode() + b'\0')
        for n in deps['consts']:
            node = self.tops.get(n)
            if node is None or isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                return None
            h.update(b'const\0' + n.encode() + b'\0' + ast.unparse(node).encode() + b'\0')
        return h.hexdigest()


def split_blocks(text, order):
    """A generator's output as (header lines, {case: its lines}); the cases
    in ORDER that wrote nothing are absent."""
    known = set(order)
    header, blocks = [], {}
    for line in text.splitlines(keepends=True):
        f = line.split(' | ')
        if len(f) > 2 and f[1] in known:
            blocks.setdefault(f[1], []).append(line)
        else:
            header.append(line)
    return header, blocks
