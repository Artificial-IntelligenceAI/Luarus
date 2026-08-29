#!/usr/bin/env python3
"""Check each implementation's time actually scales with N.

A loop the optimiser replaced with a closed form takes the same time however
large N is. That is a real and interesting result, but it is not a loop, and
reporting it as one would be a lie.
"""
import os, subprocess, sys, time

ENV = dict(os.environ, PATH="/opt/homebrew/opt/openjdk/bin:" + os.environ["PATH"])

def best(cmd, repeats=3):
    t = float("inf")
    out = ""
    for _ in range(repeats):
        s = time.perf_counter()
        p = subprocess.run(cmd, capture_output=True, text=True, env=ENV)
        t = min(t, time.perf_counter() - s)
        out = p.stdout.strip()
    return t, out

ENTRIES = {
    "A": [("C", ["./sum_c"]), ("Rust", ["./sum_rs"]), ("Go", ["./sum_go"]),
          ("Java 26", ["java", "Sum"]), ("LuaJIT", ["luajit", "sum.lua"]),
          ("Lua 5.5", ["lua", "sum.lua"]), ("Python", ["python3", "sum.py"]),
          ("NumPy", ["python3", "sum_numpy.py"])],
    "B": [("C", ["./chain_c"]), ("Rust", ["./chain_rs"]), ("Go", ["./chain_go"]),
          ("Java 26", ["java", "Chain"]), ("LuaJIT", ["luajit", "chain.lua"]),
          ("Lua 5.5", ["lua", "chain.lua"]), ("Python", ["python3", "chain.py"])],
}

for key, entries in ENTRIES.items():
    print(f"\n=== benchmark {key}: does the time scale with N? ===")
    print(f"{'':10} {'N=25M':>9} {'N=100M':>9} {'ratio':>8}   verdict")
    for label, cmd in entries:
        t1, o1 = best(cmd + ["25000000"], 3)
        t2, o2 = best(cmd + ["100000000"], 3)
        ratio = t2 / t1 if t1 > 0 else 0
        verdict = "real loop" if ratio > 2.5 else "LOOP ELIMINATED"
        print(f"{label:10} {t1:>8.4f}s {t2:>8.4f}s {ratio:>7.1f}x   {verdict}")
