//! The Luarus front end: type checking and code generation.

pub mod codegen;
pub mod literal;
pub mod typeck;

use luarus_bytecode::Chunk;
use luarus_syntax::Diagnostic;

/// Compile source text to a chunk, reporting every error found.
pub fn compile(src: &str, source_name: &str) -> Result<Chunk, Vec<Diagnostic>> {
    let program = luarus_syntax::parse(src)?;
    let checked = typeck::check_program(src, &program)?;
    Ok(codegen::emit(source_name, &checked))
}

/// Type-check without generating code.
pub fn check(src: &str) -> Result<(), Vec<Diagnostic>> {
    check_tree(src).map(|_| ())
}

/// Parse and type-check, returning the checked tree.
///
/// The reference interpreter runs this directly, so that it shares a front end
/// with the compiler and differs only in the back end.
pub fn check_tree(src: &str) -> Result<typeck::Checked, Vec<Diagnostic>> {
    let program = luarus_syntax::parse(src)?;
    typeck::check_program(src, &program)
}
