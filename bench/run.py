#!/usr/bin/env python3
"""Run every implementation of one benchmark and compare wall times.

Every command must print the same answer before any timing is reported: a
benchmark whose implementations disagree is measuring different work.
"""
import os, subprocess, sys, time

JAVA = "/opt/homebrew/opt/openjdk/bin"
ENV = dict(os.environ, PATH=JAVA + ":" + os.environ["PATH"])

def bench(name, n, entries, repeats=3):
    print(f"\n=== {name}   N = {n:,} ===")
    results, expected = [], None
    for label, cmd in entries:
        cmd = [c.replace("{n}", str(n)) for c in cmd]
        best, out = None, None
        for _ in range(repeats):
            t = time.perf_counter()
            p = subprocess.run(cmd, capture_output=True, text=True, env=ENV)
            dt = time.perf_counter() - t
            if p.returncode != 0:
                out = f"FAILED: {p.stderr.strip()[:80]}"
                best = float("inf")
                break
            out = p.stdout.strip()
            best = dt if best is None else min(best, dt)
        if expected is None and not str(out).startswith("FAILED"):
            expected = out
        agrees = "" if out == expected else f"  <-- printed {out!r}, others {expected!r}"
        results.append((label, best, agrees))

    fastest = min(r[1] for r in results if r[1] != float("inf"))
    print(f"{'':22} {'best of %d' % repeats:>10}   {'vs fastest':>10}")
    for label, best, note in sorted(results, key=lambda r: r[1]):
        if best == float("inf"):
            print(f"{label:22} {'--':>10}   {note}")
        else:
            print(f"{label:22} {best:>9.3f}s   {best/fastest:>9.1f}x{note}")

N = int(sys.argv[1]) if len(sys.argv) > 1 else 100_000_000

bench("A  sum += i   (vectorisable, and closed-form-able)", N, [
    ("C (clang -O2)",     ["./sum_c", "{n}"]),
    ("Rust (rustc -O)",   ["./sum_rs", "{n}"]),
    ("Go",                ["./sum_go", "{n}"]),
    ("Java 26",           ["java", "Sum", "{n}"]),
    ("LuaJIT",            ["luajit", "sum.lua", "{n}"]),
    ("Lua 5.5",           ["lua", "sum.lua", "{n}"]),
    ("NumPy (no loop)",   ["python3", "sum_numpy.py", "{n}"]),
    ("Python 3.9",        ["python3", "sum.py", "{n}"]),
    ("Luarus",            ["../target/release/luarus", "run", "sum.lrs"]),
])

bench("B  sum = (sum+i) %% M   (a dependent chain)", N, [
    ("C (clang -O2)",     ["./chain_c", "{n}"]),
    ("Rust (rustc -O)",   ["./chain_rs", "{n}"]),
    ("Go",                ["./chain_go", "{n}"]),
    ("Java 26",           ["java", "Chain", "{n}"]),
    ("LuaJIT",            ["luajit", "chain.lua", "{n}"]),
    ("Lua 5.5",           ["lua", "chain.lua", "{n}"]),
    ("Python 3.9",        ["python3", "chain.py", "{n}"]),
    ("Luarus",            ["../target/release/luarus", "run", "chain.lrs"]),
])
