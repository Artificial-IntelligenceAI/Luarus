# The Luarus memory model

Decided 2026-08-29. The mechanism is built and tested in `luarus-heap`; nothing
uses it yet, because the objects it will hold do not exist. See
[Status](#status).

## Aggregates share

When records and arrays exist, assigning one to another **shares** it rather
than copying:

```luarus
var Point (a) = ... end
var Point (b) = (a) end      -- one object, two names
```

Mutating through `(b)` is visible through `(a)`. This is how Lua tables behave,
and it is what makes trees, graphs and linked structures expressible.

The price is that a cycle is constructible — `(a)` holding `(b)` while `(b)`
holds `(a)` — so reclaiming memory needs something that can see the whole graph.
Reference counting cannot: neither object's count ever reaches zero.

Primitives are unaffected. `i32`, `bool` and the rest live in slots and are
copied; only heap objects are shared.

## Objects are addressed by handle

An object lives in a slab and is named by a `Handle`: a slot index paired with a
**generation counter**. Nothing holds a pointer.

This buys three things at once:

- The collector may reuse and rearrange slots freely.
- The whole heap is ordinary safe Rust — no `unsafe` anywhere in it.
- A handle to a freed object can be *detected*. Freeing a slot bumps its
  generation, so an old handle no longer matches whatever occupies the slot now.
  The check is one integer comparison.

That last point is what makes the next section possible.

## Memory is collected, and may also be freed by hand

The normal path is **mark-and-sweep tracing collection**. The collector marks
outward from the roots — the operand stack, locals and globals — and sweeps
every live slot it did not reach. Marking is iterative, so a deeply nested
structure cannot overflow the host stack, and cycles terminate because a marked
slot is never queued twice.

Collection happens **when the collector gets to it**. A program cannot know or
rely on when an object is released, and objects alive at exit are simply never
collected. A future file or socket type will therefore have to be closed by
hand; nothing can run at a predictable moment.

A program may also release an object itself:

```luarus
free (big table) end        -- placeholder syntax; not designed yet
```

Every handle to it, including copies held elsewhere, becomes detectably dead.
Using one afterwards is a **runtime error naming the rule it broke**, in the
same way arithmetic overflow is — never a silent read of whatever moved in.
This is the point of the generation counter: manual control without the failure
mode that usually comes with it.

## Interning

Values that are immutable and cannot cycle may be interned, so that equal ones
share a single object rather than each allocating. The heap provides the table;
what gets interned is a later decision.

## What this rules out, and why that was acceptable

**Reference counting** was rejected because cycles would leak silently, and a
silent failure is the one thing the language avoids everywhere else — every
other error names a rule and points at a line.

**Ownership and borrowing**, checked at compile time, would have been the most
explicit answer and costs nothing at runtime. It was rejected on scope: a borrow
checker is more work than everything built so far, and it would change how the
language feels to write. Note this is the one choice here that is *not*
reversible — a borrow checker cannot be retrofitted, while collection strategies
can be swapped freely.

**Value semantics** for aggregates would have made cycles impossible and most of
this unnecessary, at the cost of expensive copies and no way to express a graph.

## Status

| piece | state |
| --- | --- |
| Handles with generations | built, tested |
| Slab with free-list reuse | built, tested |
| Mark-sweep collection, including cycles | built, tested |
| Manual `free` with use-after-free detection | built, tested |
| Interning table | built, tested |
| The object type itself | **blocked** on the record design |
| Wiring into the VM | blocked on the above |
| `free` syntax and its rule slug | not designed |
| When collection is triggered | not decided |

`luarus-heap` is parameterised over any `T: Trace`, so the allocator does not
care what an object contains. That is why it could be built and proven before
records are designed — but it also means nothing uses it yet.

Strings deliberately stay `Rc<str>` for now. They are immutable and cannot
cycle, so a refcount is provably sufficient and cheaper than tracing them
forever. Whether they move onto the heap once records exist is open.
