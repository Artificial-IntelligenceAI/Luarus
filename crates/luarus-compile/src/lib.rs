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
    let program = luarus_syntax::parse(src)?;
    typeck::check_program(src, &program).map(|_| ())
}
