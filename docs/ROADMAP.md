# Roadmap

v0.1 deliberately built the whole pipeline over a small language rather than
part of a pipeline over a large one. Everything below is open for design.

## Next

**Control flow.** `if`/`else` and a loop. The main question is what the block
syntax is, given `end` already closes a statement chain — a block may need a
different closer to stay readable.

**Functions.** Parameter and return annotations, a call syntax that does not
collide with `(name)` or with print's `[ ]`, and frames in the VM. This is the
largest single step.

**Blocks and lexical scope.** Today the module is one flat scope. Functions
force real nesting, shadowing rules, and slot reuse.

## After that

**Records.** Nominal or structural is still undecided; it is the biggest
remaining type-system choice.

**Arrays and maps**, with the index syntax settled against `[ ]`, which print
already uses.

**Modules.** `pub` already records exports in the chunk; nothing reads them yet.

**Explicit conversions.** With no implicit widening, the language needs a
readable cast form before real arithmetic code is comfortable. Print's automatic
stringification is currently the only exception, and a cast would let it stop
being one.

**A standard library**, and with it the question of how Luarus calls out to Rust.

## Deliberately open

- Whether `| |` stays as the grouping delimiter, and whether giving it up would
  free `|` for bitwise or logical use.
- Whether quoted literals stay universal or numbers become bare.
- Whether print keeps stringifying implicitly once explicit casts exist.
- Whether `print` stays a keyword or becomes an ordinary function once there is
  a call syntax.
- Whether unused variables and unreachable code are warnings or errors.
