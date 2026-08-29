//! Property test: generated programs must run the same on both paths.
//!
//! The corpus covers what someone thought to write down. This covers what
//! nobody did. Seeds are fixed so a failure here is reproducible, and so that
//! a regression shows up as the same failing seed rather than as flakiness.
//!
//! `luarus fuzz` runs the same check over as many programs as you like, and
//! shrinks anything it finds.

use luarus_bytecode::serialize;

#[derive(Debug, PartialEq)]
enum Outcome {
    Printed(String),
    Failed(&'static str, u32),
}

/// Run a program both ways. `None` if it does not compile, which for a
/// generated program is a fault in the generator.
fn both_ways(src: &str) -> Option<(Outcome, Outcome)> {
    let checked = luarus_compile::check_tree(src).ok()?;
    let chunk = luarus_compile::compile(src, "generated").ok()?;
    let reloaded = serialize::decode(&serialize::encode(&chunk)).ok()?;

    let compiled = match luarus_vm::run_capturing(&reloaded) {
        Ok(out) => Outcome::Printed(out),
        Err(e) => Outcome::Failed(e.rule.map(|r| r.slug()).unwrap_or("?"), e.line),
    };
    let interpreted = match luarus_interp::run_capturing(&checked) {
        Ok(out) => Outcome::Printed(out),
        Err(e) => Outcome::Failed(e.rule.slug(), e.line),
    };
    Some((compiled, interpreted))
}

/// How many programs each test generates. Kept modest so the suite stays quick;
/// `luarus fuzz 100000` is there for when it should not be.
const N: u64 = 2_000;

#[test]
fn every_generated_program_compiles() {
    // The generator has to obey the language's rules by construction. A program
    // it cannot compile means one of those rules is not being respected, and
    // that program then tests nothing.
    for seed in 0..N {
        let src = luarus_gen::program(seed);
        if let Err(diags) = luarus_compile::compile(&src, "generated") {
            let rendered: Vec<String> =
                diags.iter().map(|d| luarus_diag::render(&src, "generated", d)).collect();
            panic!("seed {seed} generated a program that will not compile:\n{src}\n{}",
                rendered.join("\n"));
        }
    }
}

#[test]
fn the_two_paths_agree_on_generated_programs() {
    for seed in 0..N {
        let src = luarus_gen::program(seed);
        let Some((compiled, interpreted)) = both_ways(&src) else {
            panic!("seed {seed} did not compile");
        };
        if compiled != interpreted {
            let smaller = luarus_gen::shrink(&src, |candidate| {
                both_ways(candidate).map(|(a, b)| a != b).unwrap_or(false)
            });
            panic!(
                "seed {seed} disagrees\n  compiled:    {compiled:?}\n  \
                 interpreted: {interpreted:?}\n\nshrunk to:\n{smaller}"
            );
        }
    }
}

#[test]
fn generation_is_reproducible() {
    // A seed that only reproduces sometimes would make every failure useless.
    for seed in [0u64, 1, 42, 9999] {
        assert_eq!(luarus_gen::program(seed), luarus_gen::program(seed));
    }
    assert_ne!(luarus_gen::program(1), luarus_gen::program(2));
}

#[test]
fn generated_programs_mostly_run_to_completion() {
    // If nearly everything trapped on its first statement, the suite would be
    // testing the trap and almost nothing else.
    let mut completed = 0;
    for seed in 0..500 {
        let src = luarus_gen::program(seed);
        if let Some((Outcome::Printed(_), _)) = both_ways(&src) {
            completed += 1;
        }
    }
    assert!(completed > 300, "only {completed}/500 programs ran to completion");
}

#[test]
fn shrinking_reduces_a_program() {
    // Shrink towards any program still containing a `print`, as a stand-in for
    // a real failure predicate.
    let src = luarus_gen::program(7);
    let smaller = luarus_gen::shrink(&src, |c| {
        c.contains("print") && luarus_compile::compile(c, "t").is_ok()
    });
    assert!(smaller.len() < src.len(), "shrinking should remove something");
    assert!(luarus_compile::compile(&smaller, "t").is_ok());
}
