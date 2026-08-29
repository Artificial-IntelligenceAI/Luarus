# Testing Luarus

Most of the suite is ordinary: unit tests beside the code, and end-to-end tests
that compile a program and assert what it prints. Those assertions are
hand-written, so they are only as good as the belief that wrote them.

The differential tests are the part that does not rely on that belief.

## The oracle

`luarus-interp` is a reference interpreter. It walks the checked syntax tree
directly — no bytecode, no jumps to patch, no operand stack, no slot numbering.
It is meant to be slow and obviously correct.

Every corpus program is then run twice:

| path | what it exercises |
| --- | --- |
| compile → encode → decode → VM | code generation, the `.lrb` format, the VM |
| checked tree → interpreter | a recursive walk over the same tree |

If the two disagree, one of them is wrong. Since they share a front end, the
fault is in the compiled path.

```bash
luarus verify examples/hello.lrs
```

## What is genuinely cross-checked, and what is not

**Shared, and therefore not tested by this:** parsing, type checking, literal
parsing, half-precision conversion, and value formatting. Both paths call the
same code, so a bug there appears identically on both sides. Reimplementing them
would only manufacture divergences that mean nothing.

**Independent, and therefore tested:**

- *control flow* — arms and conditions on one side, jumps and patched
  destinations on the other;
- *storage* — a vector of values against numbered slots and globals;
- *evaluation order* — recursion against a stack;
- *integer arithmetic* — computed in `i128` and range-checked on one side,
  `checked_*` at the target width on the other. Two routes to the same answer,
  so a wrong bound on either shows up;
- *serialisation*, since the compiled path runs a chunk that has been encoded
  and decoded on the way.

Floating point is computed at its own width by the interpreter and widened to
`f64` by the VM. IEEE 754 makes those agree for `+ - * /`, so a disagreement
there would be a real fault rather than a rounding artefact.

## The corpus

`tests/corpus/*.lrs`, plus everything in `examples/`. Drop a file in and it is
picked up. A file with `fail-` in its name must fail at run time, and both paths
must fail with the same rule on the same line; a file without it must run to
completion.

## Generated programs

`luarus-gen` writes Luarus programs from a seed. They must **type-check**, or
the exercise is pointless — an invalid program is rejected identically by both
paths and says nothing about either. So generation is type-directed: an
expression is built to order for the type that is wanted, rather than built
freely and checked afterwards.

That forces the generator to respect the language by construction: operands of
an arithmetic expression share a type, a condition is a `bool`, an unsigned
value is never negated, comparisons do not chain, names are declared once and
referenced only in scope, and a literal appears bare only where a declared type
gives it one. A generated program *may* still fail at run time — overflow,
division by zero — and that is welcome, because the two paths must then agree on
the failure too.

```bash
luarus fuzz 20000            # generate, run both ways, compare
luarus fuzz 500 --seed 4200  # from a particular seed
```

A disagreement is reduced by deleting lines and keeping any smaller version that
still disagrees, so what gets reported is usually a few lines rather than the
thirty that were generated.

The test suite runs a fixed range of seeds, so a regression shows up as the same
failing seed rather than as flakiness.

### What it has found

**Two faults in the generator**, both of them the language's sharp edges rather
than oversights:

- chained comparisons, from picking `bool` as the type to compare *at* — a
  comparison is itself a `bool`, so its operands could be comparisons;
- a print item beginning with `-`, which juxtaposition binds into the previous
  item as a subtraction. The same edge caught the hand-written corpus, in
  `17-juxtaposition.lrs`.

**One real bug in the compiler.** The constant pool deduplicated entries with
`==`, and IEEE says `0.0 == -0.0`. So a later `f64 '0'` silently reused the slot
interned earlier for `f64 '-0'` and came out negative. Constants now compare by
their bits, which also lets a NaN survive a round trip. `20-signed-zero.lrs`
keeps it fixed.

## An oracle is only as good as its corpus

This is worth stating plainly, because it was demonstrated rather than assumed.
Five bugs were injected deliberately to see whether the differential tests would
notice. Against the first corpus, two of them escaped:

- **swapping `<` for `<=`** in code generation — because no program compared two
  *equal* values, and the two operators agree everywhere else;
- **loosening an overflow bound by one** — because nothing overflowed by exactly
  one; every failing case was far past the limit.

Neither was a weakness in the oracle. Both were gaps in the corpus, and both are
now covered by `18-boundaries.lrs`, `19-just-inside.lrs` and the `just-over`
failure cases. All five injections are caught now.

The lesson generalises: when adding a feature, add the case that distinguishes
it from its near neighbour, not just the case that shows it working. Generated
programs help precisely because they do not know which cases anyone thought
were interesting.

One injected fault is *supposed* to escape: emitting a condition twice and
popping the first result changes nothing, since a Luarus condition cannot have
side effects. A test that flagged it would be wrong.
