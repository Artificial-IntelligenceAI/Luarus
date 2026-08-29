#!/usr/bin/env python3
import os, subprocess, time, tempfile

ENV = dict(os.environ, PATH="/opt/homebrew/opt/openjdk/bin:" + os.environ["PATH"])
LUARUS = "../target/release/luarus"

def best(cmd, repeats=3):
    t, out = float("inf"), ""
    for _ in range(repeats):
        s = time.perf_counter()
        p = subprocess.run(cmd, capture_output=True, text=True, env=ENV)
        t = min(t, time.perf_counter() - s)
        out = p.stdout.strip() or p.stderr.strip()[:60]
    return t, out

def luarus_src(kind, n):
    """Luarus has no argv, so N is written into the source."""
    body = ("set (sum) = (sum) + (i) end" if kind == "A"
            else "set (sum) = | (sum) + (i) | % '1000000007' end")
    text = (f"var i64 (sum) = '0' end\n"
            f"loop temp store-in i64 (i) = '1' to '{n}' {{\n  {body}\n}}\n"
            f"print[(sum) \\n] end\n")
    f = tempfile.NamedTemporaryFile("w", suffix=".lrs", delete=False)
    f.write(text); f.close()
    return f.name

ENTRIES = {
 "A": [("C (clang -O2)", ["./sum_c"]), ("Rust (rustc -O)", ["./sum_rs"]),
       ("Go 1.26", ["./sum_go"]), ("Java 26", ["java", "Sum"]),
       ("LuaJIT 2.1", ["luajit", "sum.lua"]), ("Lua 5.5", ["lua", "sum.lua"]),
       ("NumPy 2.0", ["python3", "sum_numpy.py"]), ("Python 3.9", ["python3", "sum.py"])],
 "B": [("C (clang -O2)", ["./chain_c"]), ("Rust (rustc -O)", ["./chain_rs"]),
       ("Go 1.26", ["./chain_go"]), ("Java 26", ["java", "Chain"]),
       ("LuaJIT 2.1", ["luajit", "chain.lua"]), ("Lua 5.5", ["lua", "chain.lua"]),
       ("Python 3.9", ["python3", "chain.py"])],
}
TITLE = {"A": "sum += i", "B": "sum = (sum + i) % 1000000007"}
N = 100_000_000

for kind, entries in ENTRIES.items():
    rows = []
    for label, cmd in entries:
        t, out = best(cmd + [str(N)])
        t_small, _ = best(cmd + [str(N // 4)], 2)
        rows.append((label, t, out, t / t_small if t_small else 0))
    t, out = best([LUARUS, "run", luarus_src(kind, N)])
    t_small, _ = best([LUARUS, "run", luarus_src(kind, N // 4)], 2)
    rows.append(("Luarus", t, out, t / t_small if t_small else 0))

    fastest = min(r[1] for r in rows)
    print(f"\n### {kind}:  {TITLE[kind]}      N = {N:,}\n")
    print(f"{'':18} {'best of 3':>10} {'vs C':>8} {'scaling':>9}   result")
    for label, t, out, sc in sorted(rows, key=lambda r: r[1]):
        note = "" if sc > 2.5 else "  (loop eliminated)"
        print(f"{label:18} {t:>9.3f}s {t/fastest:>7.0f}x {sc:>8.1f}x   {out}{note}")
