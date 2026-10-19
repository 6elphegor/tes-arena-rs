"""Per-case cache keys for a generator built on a CASES table
(tools/regen_check.py's PER_CASE generators).

Such a generator writes a header (lines without a case's name) and then,
for each case of CASES in order, a block of lines whose second field
(`kind | NAME | ...`) is the case's name, and runs only the cases named in
ARENA_CASES (comma-separated). A case's block depends on the code every
case runs (the generator's `main` and all it reaches), on its own entry in
CASES and all that reaches, on which of the generator's case-name tables
(a top-level tuple, list or set of case names, such as WALLS_CASES) hold
it, and on everything outside the generator (its harness modules, the
EXE, the game, the save), which regen_check's key covers. So the key of a
case changes when any of that does, and adding, removing or editing
another case (with the helpers only it uses) leaves it alone.

The analysis is by name, over the generator's top-level definitions: a
name a definition uses reaches the definition of that name. Code reached
only through strings (getattr, globals) isn't seen, so a generator for
this cache must not choose its helpers that way.
"""
import ast
import hashlib


class Cases:
    """The CASES table of a generator's source and the code each case
    reaches."""

    def __init__(self, src):
        self.src = src
        tree = ast.parse(src)
        self.defs = {}     # top-level name -> (source, names used)
        self.preamble = []  # imports and other top-level statements
        self.order = []     # case names, in CASES order
        self.entries = {}   # case name -> source of its entry
        self.tables = {}    # case-name table -> the names it holds
        cases_node = None
        for node in tree.body:
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                self.defs[node.name] = (self._seg(node), self._used(node))
            elif isinstance(node, ast.Assign) and all(isinstance(t, ast.Name) for t in node.targets):
                names = [t.id for t in node.targets]
                if names == ['CASES'] and isinstance(node.value, ast.Dict):
                    cases_node = node.value
                    continue
                for n in names:
                    self.defs[n] = (self._seg(node), self._used(node.value))
            else:
                self.preamble.append(self._seg(node))
        if cases_node is None:
            raise ValueError('no CASES = {...} table')
        for k, v in zip(cases_node.keys, cases_node.values):
            if not (isinstance(k, ast.Constant) and isinstance(k.value, str)):
                raise ValueError('a CASES key that is not a string')
            self.order.append(k.value)
            self.entries[k.value] = (self._seg(k) + ': ' + self._seg(v), self._used(v))
        case_set = set(self.order)
        for n, (text, _) in list(self.defs.items()):
            node = ast.parse(text).body[0]
            value = getattr(node, 'value', None)
            if isinstance(value, (ast.Tuple, ast.List, ast.Set)) and value.elts and all(
                    isinstance(e, ast.Constant) and e.value in case_set for e in value.elts):
                self.tables[n] = {e.value for e in value.elts}
                del self.defs[n]

    def _seg(self, node):
        return ast.get_source_segment(self.src, node)

    @staticmethod
    def _used(node):
        return {n.id for n in ast.walk(node) if isinstance(n, ast.Name)}

    def reach(self, names):
        """The top-level definitions reached from NAMES."""
        seen, todo = set(), list(names)
        while todo:
            n = todo.pop()
            if n in seen or n not in self.defs:
                continue
            seen.add(n)
            todo.extend(self.defs[n][1])
        return seen

    def common(self):
        """The code every case runs: `main` and all it reaches, and the
        top-level statements that aren't definitions."""
        return self.reach({'main'})

    def common_digest(self):
        h = hashlib.sha256()
        for s in self.preamble:
            h.update(b'pre\0' + s.encode() + b'\0')
        for n in sorted(self.common()):
            h.update(b'def\0' + n.encode() + b'\0' + self.defs[n][0].encode() + b'\0')
        # A table's name used by the common code matters, not its members
        # (those are each case's: `case_digest`).
        for n in sorted(self.tables):
            h.update(b'table\0' + n.encode() + b'\0')
        return h.hexdigest()

    def case_digest(self, name):
        """What NAME's block depends on inside the generator, beyond the
        common code."""
        h = hashlib.sha256()
        text, used = self.entries[name]
        h.update(b'case\0' + text.encode() + b'\0')
        common = self.common()
        for n in sorted(self.reach(used) - common):
            h.update(b'def\0' + n.encode() + b'\0' + self.defs[n][0].encode() + b'\0')
        for n in sorted(t for t, members in self.tables.items() if name in members):
            h.update(b'in\0' + n.encode() + b'\0')
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
