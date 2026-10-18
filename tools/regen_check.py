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

Generators whose cases are sharded (`mine(n, total)`) run through
harness/par.py on K processes; the others run their cases in order on
one, as they must (gen_cast.py and gen_missile.py carry each case's
state into the next). Up to J generators run at once; each gets a hard
timeout. Identical outputs are deleted as they finish, so the scratch
directory only keeps what differs (--keep keeps everything).

A generator found identical is remembered in .regen_cache.json under a
key over everything its output can depend on: its own source and every
harness module it imports (transitively), the vector files it names, the
unpacked EXE, the game folder's file list (names, sizes, times) and the
save folder's bytes, plus its committed vector file(s). When none of
those changed it isn't run again (`cached` in the listing); --no-cache
runs everything.

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

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HARNESS = os.path.join(ROOT, 'harness')
VECTORS = os.path.join(ROOT, 'tests', 'vectors')

# Generators whose vector file isn't tests/vectors/NAME.txt.
VECTOR = {'move': 'movement', 'savew': 'save_write'}
# Vector files a generator writes beside its OUT, checked with it.
BESIDE = {'screens': ['screenfades.txt'], 'createshot': ['createframes.txt'], 'dungeonscreens': ['dungeonfades.txt']}
CACHE = os.path.join(ROOT, '.regen_cache.json')
CACHE_LOCK = threading.Lock()


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


def cache_key(name, env, vecs):
    """The key over everything gen_NAME.py's output can depend on."""
    h = hashlib.sha256()
    srcs = harness_deps(name)
    text = ''
    for path in srcs:
        data = open(path, 'rb').read()
        text += data.decode('latin-1')
        h.update(os.path.basename(path).encode() + b'\0' + data)
    # Vector files the harness reads (exclusion lists, segment maps, ...).
    for f in sorted(os.listdir(VECTORS)):
        if f in text:
            h.update(b'read ' + f.encode() + b'\0' + open(os.path.join(VECTORS, f), 'rb').read())
    for path in vecs:
        h.update(b'vec ' + os.path.basename(path).encode() + b'\0' + open(path, 'rb').read())
    h.update(open(env['exe'], 'rb').read())
    h.update(env['game_listing'])
    h.update(env['save_digest'].encode())
    h.update(env['slot'].encode())
    return h.hexdigest()


def cache_load():
    try:
        return json.load(open(CACHE))
    except (OSError, ValueError):
        return {}


def cache_store(name, key):
    with CACHE_LOCK:
        c = cache_load()
        c[name] = key
        tmp = CACHE + '.tmp'
        json.dump(c, open(tmp, 'w'), indent=0, sort_keys=True)
        os.replace(tmp, CACHE)


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
    key = cache_key(name, env, [v for v in vecs if os.path.exists(v)])
    if not opts.no_cache and env['cache'].get(name) == key:
        return name, 0.0, 'cached'
    out = os.path.join(opts.out, os.path.basename(vec))
    log = open(out + '.log', 'w')
    gen = os.path.join(HARNESS, f'gen_{name}.py')
    cmd = [sys.executable, os.path.join(HARNESS, 'par.py'), str(opts.shards), gen] if sharded \
        else [sys.executable, gen]
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
        'cache': cache_load(),
    }
    env['game_listing'] = tree_listing(env['game'])
    print(f'output in {opts.out}', flush=True)
    bad = 0
    with ThreadPoolExecutor(opts.jobs) as pool:
        for f in as_completed([pool.submit(run, n, env, opts) for n in names]):
            name, dt, res = f.result()
            print(f'{name:14} {dt:7.1f} s  {res}', flush=True)
            bad += not (res in ('identical', 'cached') or res.startswith('skipped'))
    if save:
        shutil.rmtree(save, ignore_errors=True)
    if not opts.keep and not os.listdir(opts.out):
        os.rmdir(opts.out)
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
