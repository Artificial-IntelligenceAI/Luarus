# Luarus

**Lua, but explicitly typed.**

Luarus takes Lua's minimalism and removes its guesswork. Where Lua has one
`number`, Luarus has eleven numeric types and makes you say which you meant.
Where Lua infers, Luarus asks. The compiler is written in Rust and the pipeline
is Java-shaped: source compiles to a `.lrb` bytecode chunk, and the chunk runs
on the Luarus VM.

```luarus
var f16 (x) = '1000' end
print (x) end
```

That declares a variable **named `x`**, of type `f16`, holding **1000**.

## The three rules

Luarus looks unlike Lua because of three decisions that apply everywhere.

**1. Names live in parentheses.** `(x)` is an identifier. The text between the
parens is raw, so a name can be anything you can type:

```luarus
var str (a friendly greeting) = 'hello' end
var i32 (🎯 score)            = '7'     end
var f64 (Δt)                  = '0.016' end
```

**2. Values live in quotes, and the type decides what they mean.** A literal has
no type of its own. The same four characters are an integer, a float, or text
depending on the annotation above them:

```luarus
var i32 (a) = '1000' end    -- the integer 1000
var f64 (b) = '1000' end    -- the float 1000.0
var str (c) = '1000' end    -- the text "1000"
```

This is why the annotation is mandatory rather than decorative: without it there
is nothing to read the literal *as*. A literal with no type in reach is a
compile error, not a guess.

**3. `end` terminates every statement.** Not just blocks — everything. It is
Luarus's semicolon.

Because `( ... )` is opaque, it cannot also mean grouping. Grouping is `[ ... ]`:

```luarus
var i32 (total) = [ '2' + '3' ] * '4' end   -- 20
```

## Types

```
i8   i16  i32  i64      signed integers
u8   u16  u32  u64      unsigned integers
f16  f32  f64           IEEE 754 binary16, binary32, binary64
bool  str  nil
```

There is no implicit conversion between any two of them, not even widening.
`f16` is real half precision, not `f32` wearing a label: it rounds after every
operation, so `2049` stores as `2048.0` and `70000` will not compile.

Integer arithmetic **traps on overflow** rather than wrapping. Floating point
follows IEEE 754, so it produces infinities and NaN instead.

## Scope

Bindings are local by default. Modifiers go *before* `var` and stack:

```luarus
var        i32 (n)       = '1'     end   -- local to the module
global var u8  (counter) = '0'     end   -- module global
pub    var str (version) = '0.1.0' end   -- module global, and exported
```

## Errors

The point of writing the types down is that the compiler can then be specific.

```
error: `300` is out of range for `u8`
 --> app.lrs:1:18
  |
1 | var u8 (small) = '300' end
  |                  ^^^^^
  = help: `u8` holds values from 0 to 255
```

```
error: `(cont)` is not declared
 --> app.lrs:2:7
  |
2 | print (cont) end
  |       ^^^^^^
  = help: a variable named `(count)` is declared; did you mean that?
```

## Using it

```bash
cargo build --release
```

```bash
./target/release/luarus run examples/hello.lrs
```

| command | what it does |
| --- | --- |
| `luarus run <file>` | compile if needed, then execute |
| `luarus build <file.lrs> -o <out.lrb>` | compile to bytecode |
| `luarus check <file.lrs>` | type-check only |
| `luarus dis <file>` | disassemble, in the spirit of `javap -c` |

`luarus dis` shows what the compiler actually decided:

```
  code:
      0  2  const          0    -- f16    1000 (0x63d0)
      1  2  store.local    0    -- (x)
      2  3  load.local     0    -- (x)
      3  3  print.f16
      4  3  halt
```

## Layout

| crate | contents |
| --- | --- |
| `luarus-syntax` | lexer, AST, parser, diagnostics |
| `luarus-bytecode` | value types, instructions, chunks, the `.lrb` format, `f16` |
| `luarus-compile` | type checker and code generation |
| `luarus-vm` | the virtual machine |
| `luarus-cli` | the `luarus` binary |

No third-party dependencies — the whole toolchain is `std` only.

## Status

v0.1 is a complete pipeline over a deliberately small language: declarations,
assignment, `print`, arithmetic, and comparison. Control flow, functions,
records and modules are not implemented yet. See [`docs/SPEC.md`](docs/SPEC.md)
for the language as it currently stands and [`docs/ROADMAP.md`](docs/ROADMAP.md)
for what comes next.

## Licence

Apache 2.0. See [`LICENSE`](LICENSE).
