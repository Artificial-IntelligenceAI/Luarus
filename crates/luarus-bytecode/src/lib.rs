//! The Luarus bytecode: value types, typed instructions, chunks, and the
//! `.lrb` container format.
//!
//! Compilation is Java-shaped. `luarus build` turns a `.lrs` source file into a
//! `.lrb` chunk, and `luarus run` executes that chunk on the Luarus VM. Nothing
//! about the source survives into the chunk except debug line numbers and the
//! names of globals.

pub mod chunk;
pub mod f16;
pub mod op;
pub mod serialize;
pub mod value;

pub use chunk::Chunk;
pub use op::Op;
pub use value::{Const, RtType};
