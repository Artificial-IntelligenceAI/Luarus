//! The `luarus` command line driver.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use luarus_bytecode::serialize;
use luarus_bytecode::Chunk;
use luarus_syntax::diag::render;
use luarus_syntax::Diagnostic;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
luarus — Lua, but explicitly typed

usage:
  luarus run   <file.lrs | file.lrb>     compile if needed, then execute
  luarus build <file.lrs> [-o <out>]     compile to a .lrb bytecode file
  luarus check <file.lrs>                type-check only, emit nothing
  luarus dis   <file.lrs | file.lrb>     disassemble, in the spirit of javap -c
  luarus interp <file.lrs>               run on the reference interpreter
  luarus verify <file.lrs>               run both ways and report whether they agree
  luarus rules                           list every rule the compiler enforces
  luarus version
  luarus help
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(msg)) => {
            eprintln!("error: {msg}\n\n{HELP}");
            ExitCode::from(2)
        }
        Err(Failure::Message(msg)) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
        Err(Failure::Reported) => ExitCode::FAILURE,
    }
}

enum Failure {
    Usage(String),
    Message(String),
    Reported,
}

fn dispatch(args: &[String]) -> Result<(), Failure> {
    let Some(cmd) = args.first().map(String::as_str) else {
        println!("{HELP}");
        return Ok(());
    };

    match cmd {
        "help" | "-h" | "--help" => {
            println!("{HELP}");
            Ok(())
        }
        "version" | "-V" | "--version" => {
            println!("luarus {VERSION}");
            Ok(())
        }
        "run" => cmd_run(&args[1..]),
        "build" => cmd_build(&args[1..]),
        "check" => cmd_check(&args[1..]),
        "dis" => cmd_dis(&args[1..]),
        "interp" => cmd_interp(&args[1..]),
        "verify" => cmd_verify(&args[1..]),
        "rules" => cmd_rules(),
        other => Err(Failure::Usage(format!("unknown command `{other}`"))),
    }
}

fn one_path(args: &[String], cmd: &str) -> Result<PathBuf, Failure> {
    match args {
        [p] => Ok(PathBuf::from(p)),
        [] => Err(Failure::Usage(format!("`{cmd}` needs a file"))),
        _ => Err(Failure::Usage(format!("`{cmd}` takes exactly one file"))),
    }
}

fn read_source(path: &Path) -> Result<String, Failure> {
    std::fs::read_to_string(path)
        .map_err(|e| Failure::Message(format!("could not read {}: {e}", path.display())))
}

/// Compile a source file, printing every diagnostic before giving up.
fn compile_file(path: &Path) -> Result<Chunk, Failure> {
    let src = read_source(path)?;
    let name = path.display().to_string();
    luarus_compile::compile(&src, &name).map_err(|diags| report(&src, &name, &diags))
}

fn report(src: &str, name: &str, diags: &[Diagnostic]) -> Failure {
    let mut err = std::io::stderr().lock();
    for d in diags {
        let _ = write!(err, "{}", render(src, name, d));
        let _ = writeln!(err);
    }
    let n = diags.len();
    let _ = writeln!(err, "{n} error{} found", if n == 1 { "" } else { "s" });
    Failure::Reported
}

fn load_chunk(path: &Path) -> Result<Chunk, Failure> {
    if path.extension().and_then(|e| e.to_str()) == Some("lrb") {
        let bytes = std::fs::read(path)
            .map_err(|e| Failure::Message(format!("could not read {}: {e}", path.display())))?;
        serialize::decode(&bytes)
            .map_err(|e| Failure::Message(format!("{}: {e}", path.display())))
    } else {
        compile_file(path)
    }
}

fn cmd_run(args: &[String]) -> Result<(), Failure> {
    let path = one_path(args, "run")?;
    let chunk = load_chunk(&path)?;
    let mut out = std::io::stdout().lock();
    luarus_vm::run(&chunk, &mut out).map_err(|e| Failure::Message(e.to_string()))
}

fn cmd_check(args: &[String]) -> Result<(), Failure> {
    let path = one_path(args, "check")?;
    let src = read_source(&path)?;
    let name = path.display().to_string();
    match luarus_compile::check(&src) {
        Ok(()) => {
            println!("ok: {name}");
            Ok(())
        }
        Err(diags) => Err(report(&src, &name, &diags)),
    }
}

fn cmd_dis(args: &[String]) -> Result<(), Failure> {
    let path = one_path(args, "dis")?;
    let chunk = load_chunk(&path)?;
    print!("{}", chunk.disassemble());
    Ok(())
}

/// Parse and check, returning the tree the interpreter walks.
fn checked_file(path: &Path) -> Result<luarus_compile::typeck::Checked, Failure> {
    let src = read_source(path)?;
    let name = path.display().to_string();
    luarus_compile::check_tree(&src).map_err(|diags| report(&src, &name, &diags))
}

fn cmd_interp(args: &[String]) -> Result<(), Failure> {
    let path = one_path(args, "interp")?;
    let checked = checked_file(&path)?;
    let mut out = std::io::stdout().lock();
    luarus_interp::run(&checked, &mut out).map_err(|e| Failure::Message(e.to_string()))
}

/// Run a program on both the VM and the reference interpreter and compare.
///
/// The two share a front end and differ in the whole back end, so agreement is
/// evidence about codegen, the chunk format and the VM rather than about the
/// parser or the type checker.
fn cmd_verify(args: &[String]) -> Result<(), Failure> {
    let path = one_path(args, "verify")?;
    let checked = checked_file(&path)?;
    let chunk = compile_file(&path)?;

    // Round-trip the chunk, so the container format is under test too.
    let reloaded = serialize::decode(&serialize::encode(&chunk))
        .map_err(|e| Failure::Message(format!("{}: {e}", path.display())))?;

    let compiled = luarus_vm::run_capturing(&reloaded);
    let interpreted = luarus_interp::run_capturing(&checked);

    match (compiled, interpreted) {
        (Ok(a), Ok(b)) if a == b => {
            println!("agree: {} bytes of output", a.len());
            Ok(())
        }
        (Ok(a), Ok(b)) => Err(Failure::Message(format!(
            "DISAGREE on output
  compiled:    {a:?}
  interpreted: {b:?}"
        ))),
        (Err(a), Err(b)) if a.rule == Some(b.rule) && a.line == b.line => {
            println!("agree: both fail with [{}] at line {}", b.rule.slug(), b.line);
            Ok(())
        }
        (Err(a), Err(b)) => Err(Failure::Message(format!(
            "DISAGREE on failure
  compiled:    {a}
  interpreted: {b}"
        ))),
        (Ok(a), Err(b)) => Err(Failure::Message(format!(
            "DISAGREE: compiled produced {a:?}, interpreted failed with {b}"
        ))),
        (Err(a), Ok(b)) => Err(Failure::Message(format!(
            "DISAGREE: compiled failed with {a}, interpreted produced {b:?}"
        ))),
    }
}

/// Every rule an error can cite, so the set can be read without hitting them.
fn cmd_rules() -> Result<(), Failure> {
    let width = luarus_syntax::Rule::ALL.iter().map(|r| r.slug().len()).max().unwrap_or(0);
    for rule in luarus_syntax::Rule::ALL {
        println!("{:<width$}  {}", rule.slug(), rule.statement(), width = width);
    }
    Ok(())
}

fn cmd_build(args: &[String]) -> Result<(), Failure> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| Failure::Usage("`-o` needs a path".into()))?;
                output = Some(PathBuf::from(v));
            }
            other if other.starts_with('-') => {
                return Err(Failure::Usage(format!("unknown option `{other}`")))
            }
            other => {
                if input.is_some() {
                    return Err(Failure::Usage("`build` takes exactly one input file".into()));
                }
                input = Some(PathBuf::from(other));
            }
        }
        i += 1;
    }

    let input = input.ok_or_else(|| Failure::Usage("`build` needs a file".into()))?;
    let output = output.unwrap_or_else(|| input.with_extension("lrb"));

    let chunk = compile_file(&input)?;
    let bytes = serialize::encode(&chunk);
    std::fs::write(&output, &bytes)
        .map_err(|e| Failure::Message(format!("could not write {}: {e}", output.display())))?;

    println!("wrote {} ({} bytes)", output.display(), bytes.len());
    Ok(())
}
