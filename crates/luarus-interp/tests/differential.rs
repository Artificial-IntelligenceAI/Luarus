//! Differential tests: every corpus program is run twice and the results are
//! compared.
//!
//! One path compiles to bytecode, encodes it, decodes it again and executes it
//! on the VM. The other walks the checked tree directly. They share a front end
//! and nothing else, so a disagreement is a fault in code generation, the chunk
//! format or the VM — the parts with jumps to patch, slots to number and a
//! stack to keep balanced.
//!
//! Add a case by dropping a `.lrs` file into `tests/corpus/`. A file with
//! `fail-` in its name is expected to fail at run time, and the two paths must
//! fail in the same way.

use std::path::{Path, PathBuf};

use luarus_bytecode::serialize;

fn programs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Vec::new();
    // The examples are included too, so they cannot rot unnoticed.
    for dir in ["tests/corpus", "examples"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else { continue };
        found.extend(
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("lrs")),
        );
    }
    found.sort();
    found
}

/// What one program did, on one of the two paths.
#[derive(Debug, PartialEq)]
enum Outcome {
    Printed(String),
    /// Compared by rule and line; the wording of a message is not a promise.
    Failed(&'static str, u32),
}

fn compiled(src: &str, name: &str) -> Outcome {
    let chunk = luarus_compile::compile(src, name).expect("corpus programs must compile");
    let reloaded = serialize::decode(&serialize::encode(&chunk)).expect("chunk round trip");
    match luarus_vm::run_capturing(&reloaded) {
        Ok(out) => Outcome::Printed(out),
        Err(e) => Outcome::Failed(e.rule.expect("a runtime fault cites a rule").slug(), e.line),
    }
}

fn interpreted(src: &str) -> Outcome {
    let checked = luarus_compile::check_tree(src).expect("corpus programs must check");
    match luarus_interp::run_capturing(&checked) {
        Ok(out) => Outcome::Printed(out),
        Err(e) => Outcome::Failed(e.rule.slug(), e.line),
    }
}

#[test]
fn the_two_paths_agree_on_every_corpus_program() {
    let programs = programs();
    assert!(programs.len() >= 20, "the corpus looks unexpectedly small");

    for path in &programs {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(path).expect("readable");
        assert_eq!(
            compiled(&src, &name),
            interpreted(&src),
            "the compiled and interpreted runs of {name} disagree"
        );
    }
}

#[test]
fn programs_named_fail_do_fail_at_runtime() {
    // Otherwise a case meant to exercise a trap could silently stop doing so.
    for path in programs() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.contains("fail-") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("readable");
        assert!(
            matches!(interpreted(&src), Outcome::Failed(..)),
            "{name} is named as a failing case but ran to completion"
        );
    }
}

#[test]
fn programs_not_named_fail_run_to_completion() {
    for path in programs() {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if name.contains("fail-") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("readable");
        if let Outcome::Failed(rule, line) = interpreted(&src) {
            panic!("{name} failed unexpectedly with [{rule}] at line {line}");
        }
    }
}
