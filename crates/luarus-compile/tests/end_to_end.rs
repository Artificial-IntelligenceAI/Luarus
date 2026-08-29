//! End-to-end tests: source in, program output out.
//!
//! Every run goes through `encode`/`decode` rather than the in-memory chunk, so
//! the tests cover the `.lrb` container as well as the compiler and the VM.

use luarus_bytecode::serialize;

/// Compile, serialise, reload, and run — returning what the program printed.
fn run(src: &str) -> String {
    let chunk = luarus_compile::compile(src, "test.lrs").unwrap_or_else(|diags| {
        let text: Vec<String> = diags
            .iter()
            .map(|d| luarus_syntax::diag::render(src, "test.lrs", d))
            .collect();
        panic!("expected the program to compile, but:\n{}", text.join("\n"));
    });
    let reloaded = serialize::decode(&serialize::encode(&chunk)).expect("bytecode round trip");
    luarus_vm::run_capturing(&reloaded).expect("expected the program to run")
}

/// Compile and return the error messages, asserting that it failed.
fn errors(src: &str) -> Vec<String> {
    match luarus_compile::compile(src, "test.lrs") {
        Ok(_) => panic!("expected a compile error, but the program was accepted"),
        Err(diags) => diags.into_iter().map(|d| d.message).collect(),
    }
}

fn runtime_error(src: &str) -> String {
    let chunk = luarus_compile::compile(src, "test.lrs").expect("should compile");
    luarus_vm::run_capturing(&chunk).expect_err("expected a runtime error").message
}

#[test]
fn runs_the_founding_example() {
    assert_eq!(run("var f16 (x) = '1000' end print[(x)] end"), "1000.0\n");
}

#[test]
fn names_may_contain_spaces_and_emoji() {
    let out = run(
        "var str (a friendly greeting) = 'hi' end \
         var i32 (🎯 score) = '7' end \
         print[(a friendly greeting)] end \
         print[(🎯 score)] end",
    );
    assert_eq!(out, "hi\n7\n");
}

#[test]
fn names_may_contain_operators_and_keywords() {
    // The parens are opaque, so nothing inside them is ever tokenised.
    let out = run("var i32 (a + b end) = '3' end print[(a + b end)] end");
    assert_eq!(out, "3\n");
}

#[test]
fn escapes_let_a_name_contain_parentheses() {
    let out = run(r"var i32 (f\(x\)) = '9' end print[(f\(x\))] end");
    assert_eq!(out, "9\n");
}

#[test]
fn the_same_literal_reads_differently_under_different_types() {
    let out = run(
        "var i32 (a) = '1000' end \
         var str (b) = '1000' end \
         var f64 (c) = '1000' end \
         var bool (d) = 'true' end \
         print[(a)] end print[(b)] end print[(c)] end print[(d)] end",
    );
    assert_eq!(out, "1000\n1000\n1000.0\ntrue\n");
}

#[test]
fn pipes_group_because_parens_and_brackets_are_taken() {
    assert_eq!(run("var i32 (n) = | '2' + '3' | * '4' end print[(n)] end"), "20\n");
    // Without grouping the usual precedence applies.
    assert_eq!(run("var i32 (n) = '2' + '3' * '4' end print[(n)] end"), "14\n");
}

#[test]
fn modifiers_go_before_var() {
    let out = run(
        "global var u8 (counter) = '0' end \
         pub var str (version) = '0.1.0' end \
         set (counter) = (counter) + '1' end \
         print[(counter)] end print[(version)] end",
    );
    assert_eq!(out, "1\n0.1.0\n");
}

#[test]
fn pub_is_exported_and_global_is_not() {
    let chunk = luarus_compile::compile(
        "global var i32 (hidden) = '1' end pub var i32 (shown) = '2' end",
        "t.lrs",
    )
    .unwrap();
    let exported: Vec<_> =
        chunk.globals.iter().filter(|g| g.exported).map(|g| g.name.as_str()).collect();
    assert_eq!(exported, vec!["shown"]);
}

#[test]
fn comparisons_produce_bool() {
    let out = run(
        "var i32 (n) = '5' end \
         var bool (a) = (n) > '3' end \
         var bool (b) = (n) == '5' end \
         var bool (c) = (n) != '5' end \
         print[(a)] end print[(b)] end print[(c)] end",
    );
    assert_eq!(out, "true\ntrue\nfalse\n");
}

#[test]
fn strings_compare_lexicographically() {
    let out = run("var str (s) = 'apple' end var bool (b) = (s) < 'banana' end print[(b)] end");
    assert_eq!(out, "true\n");
}

#[test]
fn comments_run_to_end_of_line() {
    assert_eq!(run("-- a comment\nvar i32 (n) = '1' end -- another\nprint[(n)] end"), "1\n");
}

#[test]
fn integer_literals_accept_separators_and_radixes() {
    let out = run(
        "var i32 (a) = '1_000_000' end var u8 (b) = '0xff' end var i32 (c) = '0b1010' end \
         print[(a)] end print[(b)] end print[(c)] end",
    );
    assert_eq!(out, "1000000\n255\n10\n");
}

#[test]
fn f16_really_loses_precision() {
    // 2049 has no binary16 representation and rounds to 2048; f32 keeps it.
    assert_eq!(run("var f16 (a) = '2049' end print[(a)] end"), "2048.0\n");
    assert_eq!(run("var f32 (b) = '2049' end print[(b)] end"), "2049.0\n");
}

#[test]
fn f16_stays_rounded_after_arithmetic() {
    // The f32 carrier must not let extra precision survive an operation.
    assert_eq!(run("var f16 (a) = '2048' end set (a) = (a) + '1' end print[(a)] end"), "2048.0\n");
}

// ------------------------------------------------------------ compile errors

#[test]
fn rejects_out_of_range_literals() {
    assert!(errors("var u8 (n) = '300' end")[0].contains("out of range for `u8`"));
    assert!(errors("var i8 (n) = '-129' end")[0].contains("out of range for `i8`"));
    assert!(errors("var f16 (n) = '70000' end")[0].contains("out of range for `f16`"));
}

#[test]
fn refuses_to_convert_between_widths() {
    let e = errors("var i32 (n) = '1' end var i64 (m) = (n) end");
    assert!(e[0].contains("expected `i64`"), "{e:?}");
}

#[test]
fn rejects_a_comparison_with_no_type_on_either_side() {
    // Outside a print list a bare literal still has no type to be read as.
    assert!(errors("var bool (b) = '1' == '2' end")[0].contains("cannot tell what type"));
}

#[test]
fn names_an_undeclared_variable_rather_than_blaming_the_literal() {
    let e = errors("print[(nope)] end");
    assert!(e[0].contains("`(nope)` is not declared"), "{e:?}");
}

#[test]
fn suggests_a_near_miss() {
    let src = "var i32 (count) = '1' end print[(cont)] end";
    let diags = luarus_compile::compile(src, "t.lrs").unwrap_err();
    assert!(diags[0].help.as_deref().unwrap().contains("(count)"));
}

#[test]
fn rejects_redeclaration() {
    assert!(errors("var i32 (n) = '1' end var i32 (n) = '2' end")[0]
        .contains("already declared"));
}

#[test]
fn rejects_negating_an_unsigned_value() {
    assert!(errors("var u8 (n) = -'1' end")[0].contains("unsigned"));
}

#[test]
fn rejects_arithmetic_on_non_numbers() {
    assert!(errors("var str (a) = 'x' end var str (b) = (a) + 'y' end")[0]
        .contains("not defined for type `str`"));
}

#[test]
fn rejects_chained_comparisons() {
    assert!(errors("var i32 (n) = '1' end var bool (b) = (n) < (n) < (n) end")[0]
        .contains("cannot be chained"));
}

#[test]
fn rejects_unknown_types() {
    assert!(errors("var number (n) = '1' end")[0].contains("unknown type `number`"));
}

#[test]
fn requires_end_on_every_statement() {
    assert!(errors("var i32 (n) = '1'")[0].contains("expected `end`"));
}

#[test]
fn requires_var_after_a_modifier() {
    assert!(errors("global i32 (n) = '1' end")[0].contains("expected `var` after `global`"));
}

#[test]
fn reports_several_errors_at_once() {
    // One bad statement must not hide the ones after it.
    assert_eq!(errors("var u8 (a) = '300' end var u8 (b) = '400' end").len(), 2);
}

#[test]
fn a_declaration_cannot_refer_to_itself() {
    assert!(errors("var i32 (n) = (n) + '1' end")[0].contains("not declared"));
}

// ------------------------------------------------------------ runtime errors

#[test]
fn overflow_traps_instead_of_wrapping() {
    let e = runtime_error("var u8 (n) = '250' end set (n) = (n) + '10' end");
    assert!(e.contains("overflowed `u8`"), "{e}");
}

#[test]
fn unsigned_subtraction_below_zero_is_an_error() {
    let e = runtime_error("var u8 (n) = '0' end set (n) = (n) - '1' end");
    assert!(e.contains("below zero"), "{e}");
}

#[test]
fn integer_division_by_zero_is_an_error() {
    let e = runtime_error("var i32 (z) = '0' end var i32 (n) = '1' / (z) end");
    assert!(e.contains("division by zero"), "{e}");
}

#[test]
fn float_division_by_zero_follows_ieee() {
    assert_eq!(run("var f64 (z) = '0' end var f64 (n) = '1' / (z) end print[(n)] end"), "inf\n");
}

// ------------------------------------------------------- print, chains, escapes

#[test]
fn prints_a_juxtaposed_list() {
    let src = "var str (name) = 'Lua ripoff 🤣' end\nprint[\"Hello, \" (name)] end";
    assert_eq!(run(src), "Hello, Lua ripoff 🤣\n");
}

#[test]
fn chained_prints_supply_their_own_newlines() {
    let src = r#"print["1" \n], print["2" \n], print["3" \n] end"#;
    assert_eq!(run(src), "1\n2\n3\n");
}

#[test]
fn a_print_alone_in_its_chain_adds_a_newline() {
    assert_eq!(run("print[\"1\"] end\nprint[\"2\"] end"), "1\n2\n");
}

#[test]
fn a_chained_print_does_not_add_one() {
    // The same two prints, chained: no newline unless it is written.
    assert_eq!(run("print[\"1\"], print[\"2\"] end"), "12");
}

#[test]
fn an_empty_print_still_ends_its_line() {
    assert_eq!(run("print[] end"), "\n");
}

#[test]
fn juxtaposition_stringifies_any_type() {
    let src = "var f16 (x) = '1000' end var u8 (k) = '7' end \
               print[\"x=\" (x) \" k=\" (k)] end";
    assert_eq!(run(src), "x=1000.0 k=7\n");
}

#[test]
fn bare_escapes_are_text() {
    assert_eq!(run(r#"print["a" \t "b" \n "c"] end"#), "a\tb\nc\n");
}

#[test]
fn escapes_work_inside_quotes_too() {
    assert_eq!(run("print[\"a\\tb\"] end"), "a\tb\n");
}

#[test]
fn chains_may_mix_statement_kinds() {
    let src = "var i32 (n) = '1', set (n) = (n) + '4', print[(n) \\n] end";
    assert_eq!(run(src), "5\n");
}

#[test]
fn a_lone_statement_still_needs_end() {
    assert!(errors("var i32 (n) = '1'")[0].contains("expected `end`"));
}

#[test]
fn pipes_nest() {
    // `| |a| |` is unambiguous: a pipe where a value is expected opens a group,
    // and one where an operator is expected closes it.
    assert_eq!(run("var i32 (m) = | | '1' + '1' | * '3' | + '1' end print[(m)] end"), "7\n");
}

#[test]
fn requires_brackets_after_print() {
    assert!(errors("var i32 (n) = '1' end print (n) end")[0].contains("expected `[` after `print`"));
}

#[test]
fn reports_an_unclosed_group() {
    assert!(errors("var i32 (n) = | '1' + '2' end")[0].contains("expected `|` to close"));
}

#[test]
fn rejects_an_escape_where_a_number_belongs() {
    assert!(errors(r#"var i32 (n) = \n end"#)[0].contains("escape is `str`"));
}

#[test]
fn rejects_an_unknown_bare_escape() {
    assert!(errors(r#"print["a" \q] end"#)[0].contains("invalid escape"));
}
