# The Luarus language, v0.1

This describes Luarus as implemented. Anything not written here is not yet in
the language.

## 1. Lexical structure

### 1.1 Comments

`--` begins a comment that runs to the end of the line.

### 1.2 Identifiers

An identifier is written `( text )`. The text is taken raw: it may contain
spaces, punctuation, digits, operators, keywords and any Unicode character
including emoji. It may not be empty and may not span a line.

Three escapes are recognised inside an identifier: `\(`, `\)` and `\\`. Nothing
else is.

```luarus
(x)
(a friendly greeting)
(🎯 score)
(f\(x\))
```

Identifiers are compared by exact text. `(x)` and `(X)` are different names, and
so are `(a b)` and `(a  b)`.

### 1.3 Literals

A literal is written `'text'` or `"text"`. The two quote styles are
interchangeable and both close on their own quote character.

Escapes: `\n`, `\t`, `\r`, `\0`, `\\`, `\'`, `\"`.

A literal has **no type of its own**. Its text is interpreted according to the
type it is checked against (§3.2).

### 1.3a Bare escapes

`\n`, `\t`, `\r`, `\0` and `\\` are also values on their own, outside any
quotes. A bare escape is always `str`, whatever the surrounding context, so
`print["1" \n]` and `print["1\n"]` are the same program.

### 1.4 Words

A bare word is a run of ASCII letters, digits and `_`. Words are keywords
(`var`, `set`, `print`, `end`, `global`, `pub`) or type names. Because every
user-chosen name is parenthesised, keywords are never in conflict with names: a
variable may legitimately be called `(end)`.

### 1.5 Operators and delimiters

```
=       assignment
+ - * / %
== != < <= > >=
| ... | grouping
[ ... ] print's value list
,       statement chaining
```

The three bracket kinds each have exactly one meaning: `( )` delimits a name,
`[ ]` delimits print's list, and `| |` groups an expression.

`|` both opens and closes a group, which is unambiguous because `|` is not a
binary operator: one appearing where a value is expected opens a group, and one
appearing where an operator is expected closes it. Nesting therefore works —
`| | '1' + '1' | * '3' |` is 6. The cost is that `|` cannot later become a
bitwise or logical operator.

## 2. Statements

```
chain := statement (',' statement)* 'end'
```

Statements chain with `,`, and one `end` closes the chain. A lone statement is a
one-element chain and still requires its `end`. Chains may span lines; the
newline is not significant.

### 2.1 Declaration

```
declaration := modifier? 'var' type identifier '=' expression
modifier    := 'global' | 'pub'
```

The type precedes the name. The initialiser is checked against the declared
type. The name is not in scope inside its own initialiser, so
`var i32 (n) = (n) + '1' end` is an error.

Redeclaring a name in the same module is an error.

### 2.2 Assignment

```
assignment := 'set' identifier '=' expression
```

The name must already be declared; the value is checked against its type.

### 2.3 Print

```
print := 'print' '[' expression* ']'
```

Values are **juxtaposed**, not separated — `print["Hello, " (name)]` writes both,
in order. An empty list is allowed.

Every value is stringified, whatever its type. This is the only implicit
conversion in the language, and it is confined to this one construct; §3.1 still
holds everywhere else. A consequence is that a bare literal inside a print list
needs no annotation, because `str` is always available as its type:
`print["12"]` is simply the text `12`.

Juxtaposition binds looser than every operator, so `print[(a) - (b)]` subtracts
rather than writing `a` then `-b`. Group to force the other reading:
`print[(a) |-(b)|]`.

**Newlines.** A `print` appends a newline exactly when it is the only statement
in its chain. A chained `print` writes only what it is given, so it carries its
own `\n`:

```luarus
print["1" \n], print["2" \n], print["3" \n] end   -- 1, 2, 3 on three lines

print["1"] end                                    -- 1, then a newline
print["2"] end                                    -- 2, then a newline

print["1"], print["2"] end                        -- 12, with no newline at all
```

## 3. Types

### 3.1 The type set

| group | types |
| --- | --- |
| signed integers | `i8` `i16` `i32` `i64` |
| unsigned integers | `u8` `u16` `u32` `u64` |
| floating point | `f16` `f32` `f64` |
| other | `bool` `str` `nil` |

There is no implicit conversion between any two types, including widening.
Mixing them is a compile error.

`f16` is IEEE 754 binary16. It is stored as 16 bits and re-rounded to half
precision after every operation, so its precision loss is observable:
`'2049'` becomes `2048.0`.

### 3.2 Reading a literal

Given an expected type, the literal's text is read as follows.

| expected | accepted text |
| --- | --- |
| `str` | anything; used verbatim |
| `bool` | `true` or `false` |
| `nil` | `nil` |
| integer types | optional `+`/`-`, then decimal, or `0x`/`0o`/`0b` digits |
| float types | a decimal float, or `inf`, `-inf`, `nan` |

`_` may appear anywhere in a numeric literal and is ignored: `'1_000_000'`.

A value outside the target type's range is a compile error, so `var u8 (n) =
'300' end` never reaches the VM.

### 3.3 Bidirectional checking

Checking runs in two modes.

*Checking* an expression against a known type pushes that type inward:
declarations, assignments and both operands of arithmetic all work this way.

*Inference* is only used where no type is supplied, and it succeeds only if the
expression contains at least one declared name to take a type from. This is why
`var i32 (n) = '2' + '3' end` compiles — the `i32` flows into both literals —
while `var bool (b) = '1' == '2' end` does not.

Inside a print list the fallback is `str` rather than an error, since everything
there is stringified anyway.

## 4. Expressions

```
expression     := comparison
comparison     := additive (('=='|'!='|'<'|'<='|'>'|'>=') additive)?
additive       := multiplicative (('+'|'-') multiplicative)*
multiplicative := unary (('*'|'/'|'%') unary)*
unary          := '-' unary | primary
primary        := literal | escape | identifier | '|' expression '|'
```

Comparisons do not chain: `a < b < c` is rejected rather than reinterpreted.

Arithmetic requires a numeric type, and both operands take the same one.
Negation requires a signed or floating type — negating an unsigned value is a
compile error rather than a wrap to a huge number.

`==` and `!=` work on every type. `<`, `<=`, `>`, `>=` work on numeric types and
on `str`, which orders lexicographically by Unicode scalar value.

Comparisons produce `bool`, and at least one side must have a type of its own.

## 5. Scope and visibility

A module has one scope. Bindings are local unless a modifier says otherwise.

| form | storage | exported |
| --- | --- | --- |
| `var` | local slot | no |
| `global var` | module global | no |
| `pub var` | module global | yes |

Export is recorded in the compiled chunk. With no module system yet, nothing
consumes it.

## 6. Runtime behaviour

Integer arithmetic is checked. Overflow, underflow of an unsigned type, and
division or remainder by zero all raise a runtime error naming the line. Luarus
never wraps silently.

Floating-point arithmetic follows IEEE 754: division by zero yields an infinity
and invalid operations yield NaN. NaN compares unequal to everything, itself
included.

Reading a slot before it is assigned is a runtime error. The checker makes this
unreachable from valid source; it exists to keep the VM safe against a
hand-written chunk.

## 7. The `.lrb` format

A chunk is little-endian, beginning with the magic `LRSB` and a `u16` version,
then: source name, local slot count, local debug names, globals (name, type
tag, exported flag), the constant pool, the instruction stream, and a line
number per instruction.

Instructions are typed. There is no generic `add` that inspects its operands —
the checker has already proved they match, so `add.i32` does exactly one thing.

`write.<type>` writes one value and never appends a newline; a `print` that owns
its line emits a `write.str` of `"\n"` after its values. Juxtaposition needs no
concatenation instruction, because writing the values in order produces the same
output.

## 8. Not yet in the language

Control flow, functions, records, arrays, maps, modules and imports, generics,
explicit conversions, string operations beyond comparison and printing, and any
standard library.
