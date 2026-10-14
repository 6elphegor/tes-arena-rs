"""Run a generator in parallel: K processes, each with SHARD=k/K (see
emu.mine: the k-th contiguous block of cases), the last argument (OUT)
given as OUT.k; the parts are then joined into OUT in order.

Usage: par.py K gen_x.py ARGS... OUT

Only for generators whose case loop asks `mine` (others would each run
every case).
"""
import os
import subprocess
import sys


def main():
    k_count, gen, args = int(sys.argv[1]), sys.argv[2], sys.argv[3:]
    if 'mine(n, total)' not in open(gen).read():
        sys.exit(f'{gen} does not shard its cases (no `mine`); run it directly')
    out = args[-1]
    procs = []
    for k in range(k_count):
        env = dict(os.environ, SHARD=f'{k}/{k_count}')
        procs.append(subprocess.Popen([sys.executable, gen, *args[:-1], f'{out}.{k}'], env=env))
    codes = [p.wait() for p in procs]
    if any(codes):
        sys.exit(f'shards failed: {codes}')
    with open(out, 'wb') as o:
        for k in range(k_count):
            part = f'{out}.{k}'
            with open(part, 'rb') as f:
                o.write(f.read())
            os.remove(part)


if __name__ == '__main__':
    main()
