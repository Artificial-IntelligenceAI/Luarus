# Roadmap

v0.1 deliberately built the whole pipeline over a small language rather than
part of a pipeline over a large one. Everything below is open for design.

## Next

**Control flow.** `if`/`else` and a loop. The main question is what the block
syntax is, given `end` already terminates simple statements — a block may need a
different closer to stay readable.

**Functions.** Parameter and return annotations, a call syntax that does not
collide with `(name)`, and frames in the VM. This is the largest single step.

**Blocks and lexical scope.** Today the module is one flat scope. Functions
force real nesting, shadowing rules, and slot reuse.

## After that

**Records.** Nominal or structural is still undecided; it is the biggest
remaining type-system choice.

**Arrays and maps**, with the index syntax settled against `[ ]` grouping.

**Modules.** `pub` already records exports in the chunk; nothing reads them yet.

**Explicit conversions.** With no implicit widening, the language needs a
readable cast form before real arithmetic code is comfortable.

**A standard library**, and with it the question of how Luarus calls out to Rust.

## Deliberately open

- Whether `[ ]` stays as the grouping delimiter.
- Whether quoted literals stay universal or numbers become bare.
- Whether unused variables and unreachable code are warnings or errors.
