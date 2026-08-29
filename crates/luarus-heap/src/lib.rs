//! The Luarus object heap: generational handles, a free list, and a mark-sweep
//! collector.
//!
//! Luarus aggregates have **reference semantics** — two names can be attached to
//! one object — so cycles are constructible and a tracing collector is what
//! reclaims them. Objects live in a slab and are addressed by [`Handle`], a
//! plain index paired with a generation counter, rather than by pointer. That
//! buys three things at once: the collector can move and reuse slots freely, the
//! whole heap is ordinary safe Rust with no `unsafe`, and a handle to a freed
//! object can be *detected* rather than silently reading whatever moved in.
//!
//! Memory is normally reclaimed by [`Heap::collect`] at a moment the program
//! does not choose. A program may also [`Heap::free`] an object itself; using a
//! handle afterwards is a loud error, never undefined behaviour.
//!
//! This module is deliberately agnostic about what an object *is*: it is
//! parameterised over any `T: Trace`, so the record design can settle later
//! without disturbing the allocator.

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

/// A reference to a heap object.
///
/// Copyable and comparable, but only meaningful to the heap that issued it.
/// The generation is what makes a stale handle detectable: freeing a slot bumps
/// it, so an old handle no longer matches whatever occupies the slot now.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Handle {
    index: u32,
    generation: u32,
}

impl Handle {
    /// The slot this handle addresses. Exposed for disassembly and debugging.
    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}·{}", self.index, self.generation)
    }
}

/// Using a handle whose object is gone.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DeadHandle(pub Handle);

impl fmt::Display for DeadHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object {} has been freed", self.0)
    }
}

impl std::error::Error for DeadHandle {}

/// How the collector finds the objects an object points at.
///
/// An implementation should push every handle it holds. Missing one makes the
/// collector free a reachable object, so this is the trait to be careful in.
pub trait Trace {
    fn trace(&self, edges: &mut Vec<Handle>);
}

enum Slot<T> {
    Empty { generation: u32 },
    Full { generation: u32, value: T },
}

impl<T> Slot<T> {
    fn generation(&self) -> u32 {
        match self {
            Slot::Empty { generation } | Slot::Full { generation, .. } => *generation,
        }
    }
}

/// A slab of objects addressed by [`Handle`].
pub struct Heap<T: Trace> {
    slots: Vec<Slot<T>>,
    /// Indices of empty slots, reused before the slab grows.
    free: Vec<u32>,
    live: usize,
    /// Scratch buffers, kept between collections so a collection allocates
    /// nothing of its own.
    mark: Vec<bool>,
    work: Vec<Handle>,
    _marker: PhantomData<T>,
}

impl<T: Trace> Default for Heap<T> {
    fn default() -> Self {
        Heap::new()
    }
}

impl<T: Trace> Heap<T> {
    pub fn new() -> Self {
        Heap {
            slots: Vec::new(),
            free: Vec::new(),
            live: 0,
            mark: Vec::new(),
            work: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// How many objects are currently alive.
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// How many slots the slab holds, live and empty together.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Put an object on the heap.
    pub fn alloc(&mut self, value: T) -> Handle {
        self.live += 1;
        if let Some(index) = self.free.pop() {
            // Reuse an empty slot. Its generation already moved on when it was
            // emptied, so handles to the previous occupant stay invalid.
            let generation = self.slots[index as usize].generation();
            self.slots[index as usize] = Slot::Full { generation, value };
            return Handle { index, generation };
        }
        let index = self.slots.len() as u32;
        self.slots.push(Slot::Full { generation: 0, value });
        Handle { index, generation: 0 }
    }

    fn slot(&self, h: Handle) -> Result<&T, DeadHandle> {
        match self.slots.get(h.index as usize) {
            Some(Slot::Full { generation, value }) if *generation == h.generation => Ok(value),
            _ => Err(DeadHandle(h)),
        }
    }

    pub fn get(&self, h: Handle) -> Result<&T, DeadHandle> {
        self.slot(h)
    }

    pub fn get_mut(&mut self, h: Handle) -> Result<&mut T, DeadHandle> {
        match self.slots.get_mut(h.index as usize) {
            Some(Slot::Full { generation, value }) if *generation == h.generation => Ok(value),
            _ => Err(DeadHandle(h)),
        }
    }

    /// Whether this handle still names a live object.
    pub fn is_live(&self, h: Handle) -> bool {
        self.slot(h).is_ok()
    }

    /// Release an object now, rather than waiting for a collection.
    ///
    /// Any handle to it — including copies held elsewhere — becomes detectably
    /// dead. This is the manual half of the memory model: real control, and a
    /// loud failure rather than a silent one when it is used wrongly.
    pub fn free(&mut self, h: Handle) -> Result<T, DeadHandle> {
        let Some(slot) = self.slots.get_mut(h.index as usize) else {
            return Err(DeadHandle(h));
        };
        match slot {
            Slot::Full { generation, .. } if *generation == h.generation => {
                let next = generation.wrapping_add(1);
                let Slot::Full { value, .. } = std::mem::replace(slot, Slot::Empty { generation: next })
                else {
                    unreachable!("just matched a full slot")
                };
                self.free.push(h.index);
                self.live -= 1;
                Ok(value)
            }
            _ => Err(DeadHandle(h)),
        }
    }

    /// Reclaim everything not reachable from `roots`. Returns how many objects
    /// were freed.
    ///
    /// Marking is iterative rather than recursive, so a deep structure cannot
    /// overflow the host stack, and cycles terminate because a marked slot is
    /// never queued twice.
    pub fn collect(&mut self, roots: &[Handle]) -> usize {
        self.mark.clear();
        self.mark.resize(self.slots.len(), false);
        let mut work = std::mem::take(&mut self.work);
        work.clear();

        for r in roots {
            if self.mark_slot(*r) {
                work.push(*r);
            }
        }

        let mut edges = Vec::new();
        while let Some(h) = work.pop() {
            edges.clear();
            if let Ok(value) = self.slot(h) {
                value.trace(&mut edges);
            }
            for e in &edges {
                if self.mark_slot(*e) {
                    work.push(*e);
                }
            }
        }
        self.work = work;

        // Sweep: empty every live slot that marking did not reach.
        let mut freed = 0;
        for index in 0..self.slots.len() {
            if self.mark[index] {
                continue;
            }
            if let Slot::Full { generation, .. } = &self.slots[index] {
                let next = generation.wrapping_add(1);
                self.slots[index] = Slot::Empty { generation: next };
                self.free.push(index as u32);
                self.live -= 1;
                freed += 1;
            }
        }
        freed
    }

    /// Mark a slot, returning whether this was the first time.
    fn mark_slot(&mut self, h: Handle) -> bool {
        let Some(slot) = self.slots.get(h.index as usize) else {
            return false;
        };
        // A stale handle among the roots is ignored rather than resurrecting
        // whatever now occupies the slot.
        if slot.generation() != h.generation || matches!(slot, Slot::Empty { .. }) {
            return false;
        }
        if self.mark[h.index as usize] {
            return false;
        }
        self.mark[h.index as usize] = true;
        true
    }

    /// Every live handle, for tests and for tooling.
    pub fn live_handles(&self) -> Vec<Handle> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| match s {
                Slot::Full { generation, .. } => {
                    Some(Handle { index: i as u32, generation: *generation })
                }
                Slot::Empty { .. } => None,
            })
            .collect()
    }
}

/// Interning table for values that are immutable and cannot cycle, so that
/// equal ones share a single object instead of each allocating.
pub struct Interner<K: Eq + Hash + Clone> {
    map: HashMap<K, Handle>,
}

impl<K: Eq + Hash + Clone> Default for Interner<K> {
    fn default() -> Self {
        Interner { map: HashMap::new() }
    }
}

impl<K: Eq + Hash + Clone> Interner<K> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &K) -> Option<Handle> {
        self.map.get(key).copied()
    }

    pub fn insert(&mut self, key: K, handle: Handle) {
        self.map.insert(key, handle);
    }

    /// Drop entries whose objects are gone, after a collection.
    pub fn retain_live<T: Trace>(&mut self, heap: &Heap<T>) {
        self.map.retain(|_, h| heap.is_live(*h));
    }

    pub fn handles(&self) -> impl Iterator<Item = Handle> + '_ {
        self.map.values().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test object that can point at other objects, which is all the
    /// collector needs to know about any object.
    #[derive(Debug, PartialEq)]
    struct Node {
        name: &'static str,
        edges: Vec<Handle>,
    }

    impl Node {
        fn new(name: &'static str) -> Node {
            Node { name, edges: Vec::new() }
        }
    }

    impl Trace for Node {
        fn trace(&self, out: &mut Vec<Handle>) {
            out.extend_from_slice(&self.edges);
        }
    }

    fn link(heap: &mut Heap<Node>, from: Handle, to: Handle) {
        heap.get_mut(from).unwrap().edges.push(to);
    }

    #[test]
    fn stores_and_returns_objects() {
        let mut heap = Heap::new();
        let a = heap.alloc(Node::new("a"));
        assert_eq!(heap.get(a).unwrap().name, "a");
        assert_eq!(heap.len(), 1);
    }

    #[test]
    fn a_freed_object_is_detected_not_reread() {
        let mut heap = Heap::new();
        let a = heap.alloc(Node::new("a"));
        heap.free(a).unwrap();
        assert_eq!(heap.get(a), Err(DeadHandle(a)));
        assert!(!heap.is_live(a));
    }

    #[test]
    fn freeing_twice_is_an_error_not_a_corruption() {
        let mut heap = Heap::new();
        let a = heap.alloc(Node::new("a"));
        assert!(heap.free(a).is_ok());
        assert!(heap.free(a).is_err());
    }

    #[test]
    fn a_reused_slot_does_not_answer_the_old_handle() {
        // The whole reason handles carry a generation: without it, `stale`
        // would silently read `b`.
        let mut heap = Heap::new();
        let stale = heap.alloc(Node::new("first"));
        heap.free(stale).unwrap();
        let b = heap.alloc(Node::new("second"));

        assert_eq!(b.index(), stale.index(), "the slot should have been reused");
        assert_ne!(b.generation(), stale.generation());
        assert_eq!(heap.get(stale), Err(DeadHandle(stale)));
        assert_eq!(heap.get(b).unwrap().name, "second");
    }

    #[test]
    fn collects_what_the_roots_cannot_reach() {
        let mut heap = Heap::new();
        let kept = heap.alloc(Node::new("kept"));
        let _dropped = heap.alloc(Node::new("dropped"));

        assert_eq!(heap.collect(&[kept]), 1);
        assert_eq!(heap.len(), 1);
        assert_eq!(heap.get(kept).unwrap().name, "kept");
    }

    #[test]
    fn keeps_what_the_roots_reach_indirectly() {
        let mut heap = Heap::new();
        let root = heap.alloc(Node::new("root"));
        let mid = heap.alloc(Node::new("mid"));
        let leaf = heap.alloc(Node::new("leaf"));
        link(&mut heap, root, mid);
        link(&mut heap, mid, leaf);

        assert_eq!(heap.collect(&[root]), 0);
        assert_eq!(heap.len(), 3);
    }

    #[test]
    fn collects_an_unreachable_cycle() {
        // This is the case reference counting cannot handle, and the reason
        // the collector traces at all.
        let mut heap = Heap::new();
        let root = heap.alloc(Node::new("root"));
        let a = heap.alloc(Node::new("a"));
        let b = heap.alloc(Node::new("b"));
        link(&mut heap, a, b);
        link(&mut heap, b, a);

        assert_eq!(heap.len(), 3);
        assert_eq!(heap.collect(&[root]), 2);
        assert_eq!(heap.len(), 1);
    }

    #[test]
    fn keeps_a_cycle_that_is_still_reachable() {
        let mut heap = Heap::new();
        let root = heap.alloc(Node::new("root"));
        let a = heap.alloc(Node::new("a"));
        let b = heap.alloc(Node::new("b"));
        link(&mut heap, root, a);
        link(&mut heap, a, b);
        link(&mut heap, b, a); // and back again

        assert_eq!(heap.collect(&[root]), 0);
        assert_eq!(heap.len(), 3);
    }

    #[test]
    fn a_self_reference_terminates() {
        let mut heap = Heap::new();
        let a = heap.alloc(Node::new("a"));
        link(&mut heap, a, a);
        assert_eq!(heap.collect(&[a]), 0);
        assert_eq!(heap.collect(&[]), 1);
    }

    #[test]
    fn a_stale_root_is_ignored_rather_than_resurrecting_a_slot() {
        let mut heap = Heap::new();
        let stale = heap.alloc(Node::new("first"));
        heap.free(stale).unwrap();
        let live = heap.alloc(Node::new("second"));

        // `stale` names the same slot as `live` but an older generation.
        assert_eq!(heap.collect(&[stale]), 1, "the new occupant is not rooted by an old handle");
        assert!(!heap.is_live(live));
    }

    #[test]
    fn collection_reuses_the_slots_it_freed() {
        let mut heap = Heap::new();
        for _ in 0..8 {
            heap.alloc(Node::new("garbage"));
        }
        assert_eq!(heap.capacity(), 8);
        heap.collect(&[]);
        assert_eq!(heap.len(), 0);

        for _ in 0..8 {
            heap.alloc(Node::new("fresh"));
        }
        assert_eq!(heap.capacity(), 8, "the slab should not have grown");
        assert_eq!(heap.len(), 8);
    }

    #[test]
    fn deep_chains_do_not_overflow_the_host_stack() {
        // Marking is iterative, so depth is bounded by the heap, not the stack.
        let mut heap = Heap::new();
        let root = heap.alloc(Node::new("root"));
        let mut prev = root;
        for _ in 0..200_000 {
            let next = heap.alloc(Node::new("link"));
            link(&mut heap, prev, next);
            prev = next;
        }
        assert_eq!(heap.collect(&[root]), 0);
        assert_eq!(heap.len(), 200_001);
    }

    #[test]
    fn collecting_after_a_manual_free_does_not_double_free() {
        let mut heap = Heap::new();
        let root = heap.alloc(Node::new("root"));
        let a = heap.alloc(Node::new("a"));
        link(&mut heap, root, a);
        heap.free(a).unwrap();

        // `root` still points at `a`, whose slot is now empty. Marking must
        // skip it rather than miscount.
        assert_eq!(heap.collect(&[root]), 0);
        assert_eq!(heap.len(), 1);
    }

    #[test]
    fn interning_shares_one_object_per_value() {
        let mut heap = Heap::new();
        let mut interner: Interner<&'static str> = Interner::new();

        let first = *interner.map_entry("hi", &mut heap);
        let second = *interner.map_entry("hi", &mut heap);
        assert_eq!(first, second);
        assert_eq!(heap.len(), 1);
    }

    impl Interner<&'static str> {
        /// Test helper: look the key up, allocating it the first time.
        fn map_entry(&mut self, key: &'static str, heap: &mut Heap<Node>) -> &Handle {
            if self.get(&key).is_none() {
                let h = heap.alloc(Node::new(key));
                self.insert(key, h);
            }
            self.map.get(&key).unwrap()
        }
    }
}
