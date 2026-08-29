# Roadmap

v0.1 deliberately built the whole pipeline over a small language rather than
part of a pipeline over a large one. Everything below is open for design.

## Next

**Loops.** `if`/`else` is in; blocks are braced, so a loop has its syntax
waiting for it. The open questions are `while` versus `for` versus both, and
what `break` looks like.

**Functions.** Parameter and return annotations, a call syntax that does not
collide with `(name)` or with print's `[ ]`, and frames in the VM. This is the
largest single step.

**Blocks and lexical scope.** Today the module is one flat scope. Functions
force real nesting, shadowing rules, and slot reuse.

## After that

**Records.** Nominal or structural is still undecided; it is the biggest
remaining type-system choice. It now also gates the memory model: `luarus-heap`
is built and tested but has nothing to hold until a record type exists. See
[MEMORY.md](MEMORY.md).

**Arrays and maps**, with the index syntax settled against `[ ]`, which print
already uses.

**Modules.** `pub` already records exports in the chunk; nothing reads them yet.

**Explicit conversions.** With no implicit widening, the language needs a
readable cast form before real arithmetic code is comfortable — and loops will
make it urgent, since a counter and whatever it indexes must otherwise share a
type exactly. Typed literals (`f16 '5'`) already give the syntax a shape to
follow. Print's automatic
stringification is currently the only exception, and a cast would let it stop
being one.

**A standard library**, and with it the question of how Luarus calls out to Rust.
String length and indexing will need the grapheme segmentation in `luarus-diag`
promoted to a real part of the language rather than a diagnostics helper.

**Memory, once records exist.** Wiring the heap into the VM, deciding what
triggers a collection, choosing the syntax for `free` and the rule its misuse
cites, and settling whether strings move onto the heap or stay refcounted.

## Deliberately open

- Whether `| |` stays as the grouping delimiter, and whether giving it up would
  free `|` for bitwise or logical use.
- Whether quoted literals stay universal or numbers become bare.
- Whether print keeps stringifying implicitly once explicit casts exist.
- Whether a shorthand for "write this and end the line" is worth having, now
  that every newline is written by hand.
- Whether `print` stays a keyword or becomes an ordinary function once there is
  a call syntax.
- Whether unused variables and unreachable code are warnings or errors.
