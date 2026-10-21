"""Regenerate every committed vector file from the original and diff it.

Usage: python tools/regen_check.py [--jobs J] [--shards K] [--timeout S]
                                   [--keep] [--no-cache] [--out DIR] [NAME...]

Runs harness/gen_NAME.py for every generator (or only the NAMEs given)
into a scratch directory and compares the result with tests/vectors/,
printing one line per generator (a generator's files beside its OUT,
BESIDE, are compared too):

  NAME  seconds  identical | cached | DIFFERENT (first line L) | rc=N | timeout | skipped: why

Run it with the Python that has unicorn (the harness's venv). It needs
re/ACD_unpacked.EXE (ARENA_UNPACKED to override), the game folder
(ARENA_GAME, default ../ESarena) and, for the save generators, a saved
game (ARENA_SAVE, slot ARENA_SLOT, default 1); the save folder is copied
to the scratch directory first, and only read from there.

Generators whose cases are sharded (`mine(n, total)`, worldframes among
them) run through harness/par.py on K processes; the others run their cases in order on
one, as they must (gen_cast.py and gen_missile.py carry each case's
state into the next). Up to J generators run at once; each gets a hard
timeout. Identical outputs are deleted as they finish, so the scratch
directory only keeps what differs (--keep keeps everything).

A generator found identical is remembered in git's common directory
(regen-cache/whole/NAME.KEY, shared by every worktree, so a fresh
worktree starts with master's results) under a key over everything its output can depend on: its own source and every
harness module it imports (transitively), the vector files it names, the
unpacked EXE, the game folder's file list (names, sizes, times) and the
save folder's bytes, plus its committed vector file(s). When none of
those changed it isn't run again (`cached` in the listing); --no-cache
runs everything.

The PER_CASE generators (worldframes) are cached case by case instead
(tools/casecache.py): each case's block of lines is kept under a key over
the generator's code that case ran last time (harness/casetrace.py
records it: blocks it never entered cut out, the constants that code
names, its own CASES entry), plus everything outside the generator as
above. Only the cases whose key changed run (through ARENA_CASES, dealt
to --shards processes longest first by their last times), and the output
is put together from the blocks in CASES order and compared with the
committed vector. So an edit to a branch or a script vocabulary reruns
only the cases that ran it, and a comment reruns none. The blocks and
each case's record of what it ran live in git's common directory
(regen-cache/), shared by every worktree. The listing then has a line
`NAME  N of M cases run` above the result.

A long run belongs in the background, e.g.

  nohup timeout 7200 venv/bin/python tools/regen_check.py > regen.log 2>&1 &
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from casecache import Cases, split_blocks  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HARNESS = os.path.join(ROOT, 'harness')
VECTORS = os.path.join(ROOT, 'tests', 'vectors')

# Generators whose vector file isn't tests/vectors/NAME.txt.
VECTOR = {'move': 'movement', 'savew': 'save_write'}
# Vector files a generator writes beside its OUT, checked with it.
BESIDE = {'screens': ['screenfades.txt'], 'createshot': ['createframes.txt'], 'dungeonscreens': ['dungeonfades.txt']}
# The whole-generator cache was once this per-worktree file; its entries
# are carried into the shared store (`cache_hit`).
OLD_CACHE = os.path.join(ROOT, '.regen_cache.json')
# Generators cached case by case (tools/casecache.py): only the cases whose
# key changed run, the rest come from the block cache.
PER_CASE = {'worldframes'}
# Sharded generators whose cases are dealt out in turn (par.py -i): one
# line per case, and slow cases bunched together (level's levels past the
# last one take a minute each, the rest a fifth of a second).
INTERLEAVED = {'level'}


def block_cache_dir(name):
    """Where a PER_CASE generator's blocks are kept: in git's common
    directory, so every worktree of the repository shares them (the blocks
    are named by their keys, so sharing is safe)."""
    try:
        common = subprocess.run(['git', 'rev-parse', '--git-common-dir'], cwd=ROOT, capture_output=True,
                                text=True, check=True).stdout.strip()
        top = os.path.join(ROOT, common, 'regen-cache')
    except (OSError, subprocess.CalledProcessError):
        top = os.path.join(ROOT, '.regen_cache')
    path = os.path.join(top, name)
    os.makedirs(path, exist_ok=True)
    return path


def write_atomic(path, text):
    tmp = f'{path}.{os.getpid()}.{threading.get_ident()}.tmp'
    with open(tmp, 'w') as f:
        f.write(text)
    os.replace(tmp, path)


def harness_deps(name):
    """gen_NAME.py and every harness module it imports, transitively."""
    seen, todo = set(), [f'gen_{name}']
    while todo:
        mod = todo.pop()
        path = os.path.join(HARNESS, mod + '.py')
        if mod in seen or not os.path.exists(path):
            continue
        seen.add(mod)
        src = open(path).read()
        for m in re.finditer(r'^\s*(?:from\s+(\w+)\s+import|import\s+([\w, ]+))', src, re.M):
            for dep in (m.group(1) or m.group(2)).split(','):
                todo.append(dep.split()[0] if dep.split() else '')
    return sorted(os.path.join(HARNESS, m + '.py') for m in seen)


def tree_listing(top):
    """Names, sizes and modification times under TOP (the game folder is
    never written, so this stands for its bytes cheaply)."""
    out = []
    for dirpath, dirs, files in os.walk(top):
        dirs.sort()
        for f in sorted(files):
            st = os.stat(os.path.join(dirpath, f))
            out.append(f'{os.path.relpath(os.path.join(dirpath, f), top)} {st.st_size} {st.st_mtime_ns}')
    return '\n'.join(out).encode()


def tree_bytes(h, top):
    for dirpath, dirs, files in os.walk(top):
        dirs.sort()
        for f in sorted(files):
            path = os.path.join(dirpath, f)
            h.update(os.path.relpath(path, top).encode() + b'\0')
            h.update(open(path, 'rb').read())


def cache_key(name, env, vecs, own=True):
    """The key over everything gen_NAME.py's output can depend on (with
    OWN false, all but gen_NAME.py's own source and its committed vectors:
    a PER_CASE generator's, which casecache keys case by case)."""
    h = hashlib.sha256()
    srcs = harness_deps(name)
    text = ''
    gen = os.path.join(HARNESS, f'gen_{name}.py')
    for path in srcs:
        data = open(path, 'rb').read()
        text += data.decode('latin-1')
        if own or path != gen:
            h.update(os.path.basename(path).encode() + b'\0' + data)
    # Vector files the harness reads (exclusion lists, segment maps, ...),
    # not those it writes.
    mine = {os.path.basename(v) for v in vecs}
    for f in sorted(os.listdir(VECTORS)):
        if f in text and f not in mine:
            h.update(b'read ' + f.encode() + b'\0' + open(os.path.join(VECTORS, f), 'rb').read())
    if own:
        for path in vecs:
            h.update(b'vec ' + os.path.basename(path).encode() + b'\0' + open(path, 'rb').read())
    h.update(open(env['exe'], 'rb').read())
    h.update(env['game_listing'])
    h.update(env['save_digest'].encode())
    h.update(env['slot'].encode())
    return h.hexdigest()


def cache_hit(name, key):
    """Whether generator NAME was found identical under KEY, in this
    worktree or any other (a key names everything the output depends on,
    so another worktree's result is as good as this one's)."""
    return os.path.exists(os.path.join(block_cache_dir('whole'), f'{name}.{key}'))


def cache_store(name, key):
    write_atomic(os.path.join(block_cache_dir('whole'), f'{name}.{key}'), '')


def cache_migrate():
    """Carry the old per-worktree .regen_cache.json into the shared store."""
    try:
        old = json.load(open(OLD_CACHE))
    except (OSError, ValueError):
        return
    for name, key in old.items():
        cache_store(name, key)
    os.remove(OLD_CACHE)


def usage(src):
    """The argument names of a generator's `Usage:` line; generators with
    none take ACD_unpacked.EXE OUT."""
    m = re.search(r'Usage: gen_\w+\.py ([^\n]*)', src)
    if not m:
        return ['ACD_unpacked.EXE', 'OUT']
    # A parenthesised note after the arguments (`(SHARD=k/K: par.py)`)
    # isn't one of them, nor the optional ones (`[--dump DIR]`).
    line = m.group(1).split('(')[0]
    while re.search(r'\[[^][]*\]', line):
        line = re.sub(r'\[[^][]*\]', '', line)
    return line.split()


def plan(name, env):
    """(argv before OUT, sharded) for generator NAME, or a reason to skip."""
    src = open(os.path.join(HARNESS, f'gen_{name}.py')).read()
    args = []
    for tok in usage(src):
        if tok == 'ACD_unpacked.EXE':
            args.append(env['exe'])
        elif tok == 'GAME_DIR':
            args.append(env['game'])
        elif tok == 'SAVE_DIR':
            if not env['save']:
                return 'needs ARENA_SAVE'
            args.append(env['save'])
        elif tok == 'SLOT':
            args.append(env['slot'])
        elif tok == 'OUT':
            pass
        else:
            return f'unknown argument {tok}'
    return args, 'mine(n, total)' in src


def run(name, env, opts):
    vec = os.path.join(VECTORS, VECTOR.get(name, name) + '.txt')
    if not os.path.exists(vec):
        return name, 0.0, f'skipped: no {os.path.relpath(vec, ROOT)}'
    p = plan(name, env)
    if isinstance(p, str):
        return name, 0.0, f'skipped: {p}'
    args, sharded = p
    vecs = [vec] + [os.path.join(VECTORS, f) for f in BESIDE.get(name, [])]
    if name in PER_CASE:
        return run_per_case(name, env, opts, vec, vecs, args, sharded)
    key = cache_key(name, env, [v for v in vecs if os.path.exists(v)])
    if not opts.no_cache and cache_hit(name, key):
        return name, 0.0, 'cached'
    out = os.path.join(opts.out, os.path.basename(vec))
    log = open(out + '.log', 'w')
    gen = os.path.join(HARNESS, f'gen_{name}.py')
    cmd = [sys.executable, os.path.join(HARNESS, 'par.py')] + (['-i'] if name in INTERLEAVED else []) \
        + [str(opts.shards), gen] if sharded else [sys.executable, gen]
    t0 = time.time()
    try:
        rc = subprocess.run(cmd + args + [out], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                            timeout=opts.timeout, start_new_session=True).returncode
    except subprocess.TimeoutExpired:
        rc = 'timeout'
    dt = time.time() - t0
    log.close()
    if rc != 0:
        return name, dt, 'timeout' if rc == 'timeout' else f'rc={rc} (see {out}.log)'
    pairs = [(out, vec)] + [(os.path.join(opts.out, f), os.path.join(VECTORS, f))
                            for f in BESIDE.get(name, [])]
    for new, old in pairs:
        if not os.path.exists(new):
            return name, dt, f'DIFFERENT (no {os.path.basename(new)} written)'
        a, b = open(new, 'rb').read(), open(old, 'rb').read()
        if a != b:
            a, b = a.split(b'\n'), b.split(b'\n')
            first = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            return name, dt, (f'DIFFERENT ({os.path.basename(old)} first line {first + 1}; '
                              f'{len(a) - 1} lines vs {len(b) - 1})')
    if not opts.keep:
        for new, _ in pairs:
            os.remove(new)
        os.remove(out + '.log')
    cache_store(name, key)
    return name, dt, 'identical'


def run_per_case(name, env, opts, vec, vecs, args, sharded):
    """A PER_CASE generator: the cases whose code changed (or that never
    ran here, or all with --no-cache) run, and the output put together
    from the blocks in CASES order is compared with the committed vector.
    A case's key is over the code it ran last time (casecache's `deps`,
    kept beside the blocks); the cases that run are dealt out to the
    processes longest first, by their last times."""
    gen = os.path.join(HARNESS, f'gen_{name}.py')
    cases = Cases(open(gen).read())
    base = cache_key(name, env, vecs, own=False)
    store = block_cache_dir(name)
    deps_dir = os.path.join(store, 'deps')
    os.makedirs(deps_dir, exist_ok=True)

    def deps_path(case):
        return os.path.join(deps_dir, (case or '_header') + '.json')

    def path_of(case, key):
        return os.path.join(store, key + ('.lines' if case else '.header'))

    def keyed(case, deps):
        k = cases.key(case, deps)
        return k and hashlib.sha256((base + k).encode()).hexdigest()

    last = {}
    for case in [None] + cases.order:
        try:
            last[case] = json.load(open(deps_path(case)))
        except (OSError, ValueError):
            pass
    keys = {}
    for case in [None] + cases.order:
        k = None if opts.no_cache or case not in last else keyed(case, last[case]['deps'])
        keys[case] = k if k and os.path.exists(path_of(case, k)) else None
    todo = [n for n in cases.order if keys[n] is None]
    header = keys[None] is None
    out = os.path.join(opts.out, os.path.basename(vec))
    dt = 0.0
    if todo or header:
        known = [last[n]['secs'] for n in cases.order if n in last]
        guess = sum(known) / len(known) if known else 60.0
        k = max(1, min(opts.shards, len(todo))) if sharded else 1
        groups, loads = [[] for _ in range(k)], [0.0] * k
        for n in sorted(todo, key=lambda n: -last.get(n, {}).get('secs', guess)):
            i = loads.index(min(loads))
            groups[i].append(n)
            loads[i] += last.get(n, {}).get('secs', guess)
        log = open(out + '.log', 'w')
        print(f'{name}: running {len(todo)} of {len(cases.order)} cases on {k}', file=log, flush=True)
        t0 = time.time()
        procs = []
        for i, group in enumerate(groups):
            run_env = dict(os.environ, ARENA_CASES=','.join(group) or '-', ARENA_TRACE=f'{out}.{i}.trace')
            run_env.pop('SHARD', None)
            run_env.pop('ARENA_HEADER', None)
            if i == 0 and header:
                run_env['ARENA_HEADER'] = '1'
            procs.append(subprocess.Popen([sys.executable, gen] + args + [f'{out}.{i}'], cwd=ROOT, stdout=log,
                                          stderr=subprocess.STDOUT, start_new_session=True, env=run_env))
        codes = []
        for p in procs:
            try:
                codes.append(p.wait(timeout=max(1, opts.timeout - (time.time() - t0))))
            except subprocess.TimeoutExpired:
                codes.append('timeout')
        if 'timeout' in codes:
            for p in procs:
                if p.poll() is None:
                    os.killpg(p.pid, 9)
                    p.wait()
        dt = time.time() - t0
        log.close()
        if any(c != 0 for c in codes):
            return name, dt, 'timeout' if 'timeout' in codes else f'rc={codes} (see {out}.log)'
        for i, group in enumerate(groups):
            part = f'{out}.{i}'
            head, blocks = split_blocks(open(part).read(), cases.order)
            trace = json.load(open(f'{part}.trace'))
            mod = {int(c[4:]): set(t['lines']) for c, t in trace.items() if c.startswith('mod:')}

            def ran(*ctxs):
                return set().union(*(trace.get(c, {}).get('lines', ()) for c in ctxs))
            if i == 0 and header:
                deps = cases.deps(None, ran(''), mod)
                keys[None] = keyed(None, deps)
                write_atomic(path_of(None, keys[None]), ''.join(head))
                write_atomic(deps_path(None), json.dumps({'deps': deps, 'secs': 0.0}))
            missing = [n for n in group if n not in blocks]
            if missing:
                return name, dt, f'DIFFERENT (no lines for {", ".join(missing[:5])}; see {out}.log)'
            for n in group:
                own = trace.get('case:' + n, {})
                deps = cases.deps(n, ran('', 'case:' + n, *own.get('uses', ())), mod)
                keys[n] = keyed(n, deps)
                write_atomic(path_of(n, keys[n]), ''.join(blocks[n]))
                write_atomic(deps_path(n), json.dumps({'deps': deps, 'secs': own.get('secs', 0.0)}))
            os.remove(part)
            os.remove(f'{part}.trace')
    text = open(path_of(None, keys[None])).read() + ''.join(open(path_of(n, keys[n])).read() for n in cases.order)
    note = f'{len(todo)} of {len(cases.order)} cases run'
    with open(out, 'w') as f:
        f.write(text)
    a, b = text.encode(), open(vec, 'rb').read()
    if a != b:
        a, b = a.split(b'\n'), b.split(b'\n')
        first = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
        return name, dt, (f'DIFFERENT ({os.path.basename(vec)} first line {first + 1}; '
                          f'{len(a) - 1} lines vs {len(b) - 1}; {note})')
    if not opts.keep:
        os.remove(out)
        if os.path.exists(out + '.log'):
            os.remove(out + '.log')
    return name, dt, ('identical' if todo else 'cached'), note


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('names', nargs='*')
    ap.add_argument('--jobs', type=int, default=2)
    ap.add_argument('--shards', type=int, default=max(1, (os.cpu_count() or 2) // 2))
    ap.add_argument('--timeout', type=int, default=1800, help='seconds per generator')
    ap.add_argument('--keep', action='store_true')
    ap.add_argument('--no-cache', action='store_true', help='run generators even when cached')
    ap.add_argument('--out')
    opts = ap.parse_args()
    names = opts.names or sorted(f[4:-3] for f in os.listdir(HARNESS)
                                 if f.startswith('gen_') and f.endswith('.py'))
    opts.out = opts.out or tempfile.mkdtemp(prefix='regen-')
    os.makedirs(opts.out, exist_ok=True)
    save = os.environ.get('ARENA_SAVE')
    digest = hashlib.sha256()
    if save:
        tree_bytes(digest, os.path.expanduser(save))
    if save:
        copy = os.path.join(opts.out, 'save')
        shutil.copytree(os.path.expanduser(save), copy, dirs_exist_ok=True)
        save = copy
    env = {
        'exe': os.environ.get('ARENA_UNPACKED', os.path.join(ROOT, 're', 'ACD_unpacked.EXE')),
        'game': os.environ.get('ARENA_GAME', os.path.join(ROOT, '..', 'ESarena')),
        'save': save,
        'slot': os.environ.get('ARENA_SLOT', '1'),
        'save_digest': digest.hexdigest() if save else '',
    }
    env['game_listing'] = tree_listing(env['game'])
    cache_migrate()
    print(f'output in {opts.out}', flush=True)
    bad = 0
    with ThreadPoolExecutor(opts.jobs) as pool:
        for f in as_completed([pool.submit(run, n, env, opts) for n in names]):
            name, dt, res, *note = f.result()
            if note:
                print(f'{name:14} {note[0]}', flush=True)
            print(f'{name:14} {dt:7.1f} s  {res}', flush=True)
            bad += not (res in ('identical', 'cached') or res.startswith('skipped'))
    if save:
        shutil.rmtree(save, ignore_errors=True)
    if not opts.keep and not os.listdir(opts.out):
        os.rmdir(opts.out)
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
