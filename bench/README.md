# Loop benchmark

Luarus against C, Rust, Go, Java, LuaJIT, Lua, NumPy and Python, on a loop.

Run on an **Apple M5** (10 cores), macOS. Best of three runs, wall clock,
including process startup.

```bash
python3 final.py     # the table below
python3 scale.py     # the scaling check that validates it
```

## Method, and the trap in it

Two benchmarks, both summing `1..N` with `N = 100,000,000`:

- **A** — `sum += i`
- **B** — `sum = (sum + i) % 1000000007`

Every implementation must print the same answer, and every implementation is
checked for **whether it still contains a loop at all**. That check matters more
than the timings: `clang -O2` recognises benchmark A as an arithmetic series and
replaces the whole loop with Gauss's formula. Its "0.002 seconds" is process
startup, not a hundred million iterations.

The test is whether the time scales with `N`. Quadrupling `N` should roughly
quadruple the time; anything near 1.0× has had its loop deleted:

| | N=25M | N=100M | ratio | |
| --- | --- | --- | --- | --- |
| C, benchmark A | 0.0016s | 0.0016s | **1.0×** | loop eliminated |
| C, benchmark B | 0.0631s | 0.2468s | 3.9× | real loop |

Benchmark B exists because of that. A dependent chain — each value needs the one
before it — cannot be closed-form, cannot be vectorised, and cannot be run out
of order. Everybody actually loops.

The other languages read `N` from `argv` so it cannot be folded; Luarus has no
argv, so its `N` is written into the source. That costs it nothing, since the
Luarus compiler does no constant folding of any kind.

## A — `sum += i`

| | best of 3 | vs C | scaling | |
| --- | --- | --- | --- | --- |
| C (clang -O2) | 0.002s | 1× | 1.1× | **loop eliminated** |
| Rust (rustc -O) | 0.013s | 7× | 2.9× | |
| Go 1.26 | 0.025s | 13× | 2.8× | |
| Java 26 | 0.046s | 25× | 1.7× | **loop eliminated** |
| LuaJIT 2.1 | 0.056s | 30× | 3.5× | |
| NumPy 2.0 | 0.077s | 41× | 1.3× | **not a loop at all** |
| Lua 5.5 | 0.213s | 114× | 3.8× | |
| **Luarus** | **1.996s** | **1071×** | 4.1× | |
| Python 3.9 | 3.295s | 1768× | 4.0× | |

Three of these are not measurements of looping. Against Rust — the fastest thing
here still running a real loop — Luarus is **153×** slower.

## B — `sum = (sum + i) % 1000000007`

| | best of 3 | vs C | scaling | |
| --- | --- | --- | --- | --- |
| C (clang -O2) | 0.249s | 1× | 3.9× | |
| Rust (rustc -O) | 0.249s | 1.0× | 3.9× | |
| Go 1.26 | 0.257s | 1.0× | 4.0× | |
| Java 26 | 0.264s | 1.1× | 3.2× | |
| Lua 5.5 | 0.393s | 1.6× | 3.9× | |
| LuaJIT 2.1 | 0.427s | 1.7× | 3.9× | |
| **Luarus** | **2.837s** | **11×** | 4.0× | |
| Python 3.9 | 3.913s | 16× | 4.1× | |

## What the two tables say together

**C, Rust, Go and Java all land within 6% of each other.** The chain is
latency-bound: every iteration waits on the previous modulo, so there is nothing
for an optimiser to overlap. Four very different compilers converge because the
hardware, not the compiler, sets the floor.

**That floor flatters every interpreter here.** Lua 5.5 is 1.6× off C on a loop
it interprets one instruction at a time — because dispatch overhead hides in the
shadow of the modulo latency. The same gap on benchmark A, where the compiled
languages can vectorise, is 114×.

So Luarus's real distance from a compiler is **11× on latency-bound work and
153× on throughput-bound work**. The first number is the flattering one.

**Lua 5.5 beats LuaJIT on B.** LuaJIT is Lua 5.1, where every number is a
double, so its `%` is a floating-point remainder; Lua 5.5 has native integers.
The JIT wins benchmark A by 4× and loses B by 10%.

## Where Luarus's time goes, and what was done about it

The first version of this benchmark measured a loop that spent seventeen
instructions per iteration, **two** of which did the arithmetic. The counter was
loaded three times and copied into the target every time round, whether or not
the body read it.

Two changes fixed most of it:

- **A fused `loop.step`**, in the shape of Lua's `FORLOOP`: step, test and
  branch in one instruction rather than nine.
- **The target doubles as the counter** when the body never assigns to it, so
  the copy happens once before the loop instead of every iteration. A body that
  *does* assign to the target gives this up, or the assignment would move the
  counter.

Seventeen instructions became seven:

```
     12      load.local     0    -- (sum)            ┐
     13      load.local     1    -- (i)              │ the actual work
     14      add.i64                                 │
     15      const          3    -- int 1000000007   │
     16      rem.i64                                 ┘
     17      store.local    0    -- (sum)
     18      loop.step.i64  1 3 12  -- (i)           the entire loop tail
```

Instruction count was only half the story. The first cut of `loop.step` went
through the VM's general arithmetic, matching on the operator and type and
building a fresh `1` every iteration — and it cost about as much as the nine
instructions it replaced, giving only 1.4×. Specialising the machine-integer
case, where nearly every loop lives, is what turned that into:

| | before | after | |
| --- | --- | --- | --- |
| A `sum += i` | 4.488s | **1.996s** | 2.25× faster |
| B dependent chain | 5.142s | **2.837s** | 1.81× faster |

Which moved Luarus from 21× to **11×** off C on the chain, and past CPython on
both benchmarks.

## Not measured

**Java's JIT warmup** is inside these numbers — the whole process is timed,
including class loading. On benchmark B that is roughly 15ms of the 264ms.

**NumPy is not looping.** It allocates an array and hands it to C. It appears in
benchmark A for scale, and cannot do benchmark B at all: a dependent chain is
exactly what array programming cannot vectorise.
