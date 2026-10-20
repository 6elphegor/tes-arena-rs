"""Run a generator in parallel: K processes, each with SHARD=k/K (see
emu.mine: the k-th contiguous block of cases), the last argument (OUT)
given as OUT.k; the parts are then joined into OUT in order.

Usage: par.py [-i] K gen_x.py ARGS... OUT

Only for generators whose case loop asks `mine` (others would each run
every case). With -i the cases are dealt out in turn (SHARD=k/K/i: every
K-th case from the k-th, which the generator must handle itself) and the joined output takes a line from each
part in turn, so it is only for generators that write one line per case;
it keeps a run of slow cases from landing on one process (gen_level's
levels past the last one).
"""
import os
import subprocess
import sys


def main():
    argv = sys.argv[1:]
    interleave = argv[0] == '-i'
    if interleave:
        argv = argv[1:]
    k_count, gen, args = int(argv[0]), argv[1], argv[2:]
    if 'mine(n, total)' not in open(gen).read():
        sys.exit(f'{gen} does not shard its cases (no `mine`); run it directly')
    out = args[-1]
    procs = []
    for k in range(k_count):
        env = dict(os.environ, SHARD=f'{k}/{k_count}' + ('/i' if interleave else ''))
        procs.append(subprocess.Popen([sys.executable, gen, *args[:-1], f'{out}.{k}'], env=env))
    codes = [p.wait() for p in procs]
    if any(codes):
        sys.exit(f'shards failed: {codes}')
    parts = [f'{out}.{k}' for k in range(k_count)]
    with open(out, 'wb') as o:
        if interleave:
            lines = [open(p, 'rb').read().splitlines(keepends=True) for p in parts]
            for i in range(max(map(len, lines))):
                for part in lines:
                    if i < len(part):
                        o.write(part[i])
        else:
            for part in parts:
                with open(part, 'rb') as f:
                    o.write(f.read())
    for part in parts:
        os.remove(part)


if __name__ == '__main__':
    main()
