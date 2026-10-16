"""Regenerate every committed vector file from the original and diff it.

Usage: python tools/regen_check.py [--jobs J] [--shards K] [--timeout S]
                                   [--keep] [--out DIR] [NAME...]

Runs harness/gen_NAME.py for every generator (or only the NAMEs given)
into a scratch directory and compares the result with tests/vectors/,
printing one line per generator (a generator's files beside its OUT,
BESIDE, are compared too):

  NAME  seconds  identical | DIFFERENT (first line L) | rc=N | timeout | skipped: why

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

A long run belongs in the background, e.g.

  nohup timeout 7200 venv/bin/python tools/regen_check.py > regen.log 2>&1 &
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HARNESS = os.path.join(ROOT, 'harness')
VECTORS = os.path.join(ROOT, 'tests', 'vectors')

# Generators whose vector file isn't tests/vectors/NAME.txt.
VECTOR = {'move': 'movement', 'savew': 'save_write'}
# Vector files a generator writes beside its OUT, checked with it.
BESIDE = {'screens': ['screenfades.txt'], 'createshot': ['createframes.txt']}


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
    return name, dt, 'identical'


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('names', nargs='*')
    ap.add_argument('--jobs', type=int, default=2)
    ap.add_argument('--shards', type=int, default=max(1, (os.cpu_count() or 2) // 2))
    ap.add_argument('--timeout', type=int, default=1800, help='seconds per generator')
    ap.add_argument('--keep', action='store_true')
    ap.add_argument('--out')
    opts = ap.parse_args()
    names = opts.names or sorted(f[4:-3] for f in os.listdir(HARNESS)
                                 if f.startswith('gen_') and f.endswith('.py'))
    opts.out = opts.out or tempfile.mkdtemp(prefix='regen-')
    os.makedirs(opts.out, exist_ok=True)
    save = os.environ.get('ARENA_SAVE')
    if save:
        copy = os.path.join(opts.out, 'save')
        shutil.copytree(os.path.expanduser(save), copy, dirs_exist_ok=True)
        save = copy
    env = {
        'exe': os.environ.get('ARENA_UNPACKED', os.path.join(ROOT, 're', 'ACD_unpacked.EXE')),
        'game': os.environ.get('ARENA_GAME', os.path.join(ROOT, '..', 'ESarena')),
        'save': save,
        'slot': os.environ.get('ARENA_SLOT', '1'),
    }
    print(f'output in {opts.out}', flush=True)
    bad = 0
    with ThreadPoolExecutor(opts.jobs) as pool:
        for f in as_completed([pool.submit(run, n, env, opts) for n in names]):
            name, dt, res = f.result()
            print(f'{name:14} {dt:7.1f} s  {res}', flush=True)
            bad += not (res == 'identical' or res.startswith('skipped'))
    if save:
        shutil.rmtree(save, ignore_errors=True)
    if not opts.keep and not os.listdir(opts.out):
        os.rmdir(opts.out)
    sys.exit(1 if bad else 0)


if __name__ == '__main__':
    main()
