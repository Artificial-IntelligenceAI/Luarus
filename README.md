# Luarus (Lua + Rust), get it?

**Lua, but explicitly typed.**

Luarus takes Lua's minimalism and removes its guesswork. Where Lua has one
`number`, Luarus has eleven numeric types and makes you say which you meant.
Where Lua infers, Luarus asks. The compiler is written in Rust and the pipeline
is Java-shaped: source compiles to a `.lrb` bytecode chunk, and the chunk runs
on the Luarus VM.

```luarus
var f16 (x) = '1000' end
print[(x) \n] end
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
var i32 (🧑‍🧑‍🧒‍🧒)                  = '4'     end
```

That last name is **one character**. Luarus counts characters the way a reader
does — as grapheme clusters — so `🧑‍🧑‍🧒‍🧒` is one, exactly like `c`, even though it
is seven Unicode scalars welded together with zero-width joiners. Error columns
count characters this way; caret alignment counts terminal cells instead, since
an emoji draws two cells wide where a space draws one.

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

**3. `end` closes a statement chain.** Statements chain with `,` and one `end`
closes the chain. A lone statement is a one-element chain and still needs its
`end`.

```luarus
var i32 (a) = '1', var i32 (b) = '2', set (a) = (b) end
```

Because `( ... )` is a name and `[ ... ]` is print's list, neither was free for
grouping. Grouping is `| ... |`, and it nests:

```luarus
var i32 (total)  = | '2' + '3' | * '4' end            -- 20
var i32 (nested) = | | '1' + '1' | * '3' | + '1' end  -- 7
```

A `|` where a value is expected opens a group and one where an operator is
expected closes it, so there is no ambiguity — at the cost of `|` never becoming
a bitwise operator.

## Choosing

`if` takes a braced block, and `else` divides that same block in two rather than
opening a second one:

```luarus
var f16 (x) = '1000' end
if (x) > f16 '5' {
print["x is greater than 5" \n] end
else print["x is less than 5" \n] end }
```

A block is a scope: a name declared inside one is gone at the `}`. There is **no
truthiness** — a condition is a `bool` or it is an error, so `if (n)` on an
integer will not compile.

`elseif` divides the same block again, so a chain of any length still closes
with a single brace:

```luarus
if (score) > '90' {
  print["grade a" \n] end
elseif (score) > '80'
  print["grade b" \n] end
elseif (score) > '70'
  print["grade c" \n] end
else
  print["grade f" \n] end }
```

## Typed literals

A literal normally takes its type from context. It can also state it outright,
which lets it stand where nothing would supply one:

```luarus
print[i32 '42' " and " f64 '1.5'] end     -- neither has a context to read
var bool (b) = i32 '1' == i32 '2' end     -- nor does either side here
```

`f16 '5'` is still a literal, not a conversion: it is checked and range-checked
exactly as a bare one is, and it may not disagree with a type already expected.

## Printing

`print` takes its values in brackets, juxtaposed rather than separated. Anything
in the list is stringified, whatever its type:

```luarus
var str (name) = 'Lua ripoff 🤣' end
print["Hello, " (name)] end          -- Hello, Lua ripoff 🤣

var f16 (x) = '1000' end
var u8  (k) = '7' end
print["x=" (x) " k=" (k)] end        -- x=1000.0 k=7
```

This is the one place Luarus converts without being told to.

`print` writes **exactly** what it is given: no separators between items, and no
newline. Every line ending is written by hand:

```luarus
print["1" \n], print["2" \n], print["3" \n] end   -- three lines

print["a"], print["b"] end                        -- ab, no newline at all
```

`\n`, `\t`, `\r`, `\0` and `\\` work as bare tokens outside the quotes, as
above, or inside them as usual.

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

Every error names the **rule** it broke. The message says what went wrong here;
the rule says what is true everywhere.

```
error[values-must-fit]: `300` is out of range for `u8`
 --> app.lrs:1:18
  |
1 | var u8 (small) = '300' end
  |                  ^^^^^
  = rule: a literal must be a valid value of the type it is read as
  = help: `u8` holds values from 0 to 255
```

```
error[names-must-be-declared]: `(cont)` is not declared
 --> app.lrs:2:7
  |
2 | print[(cont)] end
  |       ^^^^^^
  = rule: a name is declared before it is used
  = help: a variable named `(count)` is declared; did you mean that?
```

Runtime errors cite rules too:

```
runtime error[overflow-traps]: arithmetic overflowed `u8`
  --> app.lrs:2
  = rule: arithmetic never wraps; a result must fit its type
```

`luarus rules` lists the whole set — there are twenty-one, and every error cites
one of them.

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
| `luarus rules` | list every rule the compiler enforces |

`luarus dis` shows what the compiler actually decided:

```
  code:
      0      2  const          0    -- f16    1000 (0x63d0)
      1      2  store.local    0    -- (x)
      2      3  load.local     0    -- (x)
      3      3  write.f16
      4      3  const          1    -- str    "\n"
      5      3  write.str
      6      3  halt
```

## Layout

| crate | contents |
| --- | --- |
| `luarus-diag` | spans, the rule set, diagnostic rendering, grapheme segmentation |
| `luarus-heap` | the object heap: generational handles, mark-sweep collection |
| `luarus-syntax` | lexer, AST, parser |
| `luarus-bytecode` | value types, instructions, chunks, the `.lrb` format, `f16` |
| `luarus-compile` | type checker and code generation |
| `luarus-vm` | the virtual machine |
| `luarus-cli` | the `luarus` binary |

No third-party dependencies — the whole toolchain is `std` only.

## Status

v0.1 is a complete pipeline over a deliberately small language: declarations,
assignment, chained statements, `print`, arithmetic, and comparison. Control flow, functions,
records and modules are not implemented yet. See [`docs/SPEC.md`](docs/SPEC.md)
for the language as it currently stands, [`docs/MEMORY.md`](docs/MEMORY.md) for
the memory model, and [`docs/ROADMAP.md`](docs/ROADMAP.md) for what comes next.

## Licence

Apache 2.0. See [`LICENSE`](LICENSE).
