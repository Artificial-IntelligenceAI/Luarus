//! End-to-end tests: source in, program output out.
//!
//! Every run goes through `encode`/`decode` rather than the in-memory chunk, so
//! the tests cover the `.lrb` container as well as the compiler and the VM.

use luarus_bytecode::serialize;
use luarus_diag::Rule;
use luarus_syntax::Diagnostic;

/// Compile, serialise, reload, and run — returning what the program printed.
fn run(src: &str) -> String {
    let chunk = luarus_compile::compile(src, "test.lrs").unwrap_or_else(|diags| {
        let text: Vec<String> =
            diags.iter().map(|d| luarus_diag::render(src, "test.lrs", d)).collect();
        panic!("expected the program to compile, but:\n{}", text.join("\n"));
    });
    let reloaded = serialize::decode(&serialize::encode(&chunk)).expect("bytecode round trip");
    luarus_vm::run_capturing(&reloaded).expect("expected the program to run")
}

fn diags(src: &str) -> Vec<Diagnostic> {
    match luarus_compile::compile(src, "test.lrs") {
        Ok(_) => panic!("expected a compile error, but the program was accepted"),
        Err(d) => d,
    }
}

fn errors(src: &str) -> Vec<String> {
    diags(src).into_iter().map(|d| d.message).collect()
}

/// The rule cited by the first error.
fn rule(src: &str) -> Rule {
    diags(src)[0].rule
}

fn runtime_error(src: &str) -> luarus_vm::RuntimeError {
    let chunk = luarus_compile::compile(src, "test.lrs").expect("should compile");
    luarus_vm::run_capturing(&chunk).expect_err("expected a runtime error")
}

// ------------------------------------------------------------------ the basics

#[test]
fn runs_the_founding_example() {
    assert_eq!(run("var f16 (x) = '1000' end print[(x)] end"), "1000.0");
}

#[test]
fn names_may_contain_spaces_and_emoji() {
    let out = run(
        "var str (a friendly greeting) = 'hi' end \
         var i32 (🎯 score) = '7' end \
         print[(a friendly greeting) \\n (🎯 score)] end",
    );
    assert_eq!(out, "hi\n7");
}

#[test]
fn names_may_contain_operators_and_keywords() {
    // The parens are opaque, so nothing inside them is ever tokenised.
    assert_eq!(run("var i32 (a + b end) = '3' end print[(a + b end)] end"), "3");
}

#[test]
fn escapes_let_a_name_contain_parentheses() {
    assert_eq!(run(r"var i32 (f\(x\)) = '9' end print[(f\(x\))] end"), "9");
}

#[test]
fn the_same_literal_reads_differently_under_different_types() {
    let out = run(
        "var i32 (a) = '1000' end var str (b) = '1000' end \
         var f64 (c) = '1000' end var bool (d) = 'true' end \
         print[(a) \" \" (b) \" \" (c) \" \" (d)] end",
    );
    assert_eq!(out, "1000 1000 1000.0 true");
}

#[test]
fn pipes_group_because_parens_and_brackets_are_taken() {
    assert_eq!(run("var i32 (n) = | '2' + '3' | * '4' end print[(n)] end"), "20");
    // Without grouping the usual precedence applies.
    assert_eq!(run("var i32 (n) = '2' + '3' * '4' end print[(n)] end"), "14");
}

#[test]
fn pipes_nest() {
    // A pipe where a value is expected opens a group; one where an operator is
    // expected closes it, so nesting is unambiguous.
    assert_eq!(run("var i32 (m) = | | '1' + '1' | * '3' | + '1' end print[(m)] end"), "7");
}

#[test]
fn modifiers_go_before_var() {
    let out = run(
        "global var u8 (counter) = '0' end pub var str (version) = '0.1.0' end \
         set (counter) = (counter) + '1' end \
         print[(counter) \" \" (version)] end",
    );
    assert_eq!(out, "1 0.1.0");
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
        "var i32 (n) = '5' end var bool (a) = (n) > '3' end \
         var bool (b) = (n) == '5' end var bool (c) = (n) != '5' end \
         print[(a) \" \" (b) \" \" (c)] end",
    );
    assert_eq!(out, "true true false");
}

#[test]
fn strings_compare_lexicographically() {
    assert_eq!(run("var str (s) = 'apple' end print[| (s) < 'banana' |] end"), "true");
}

#[test]
fn comments_run_to_end_of_line() {
    assert_eq!(run("-- a comment\nvar i32 (n) = '1' end -- another\nprint[(n)] end"), "1");
}

#[test]
fn integer_literals_accept_separators_and_radixes() {
    let out = run(
        "var i32 (a) = '1_000_000' end var u8 (b) = '0xff' end var i32 (c) = '0b1010' end \
         print[(a) \" \" (b) \" \" (c)] end",
    );
    assert_eq!(out, "1000000 255 10");
}

#[test]
fn f16_really_loses_precision() {
    // 2049 has no binary16 representation and rounds to 2048; f32 keeps it.
    assert_eq!(run("var f16 (a) = '2049' end print[(a)] end"), "2048.0");
    assert_eq!(run("var f32 (b) = '2049' end print[(b)] end"), "2049.0");
}

#[test]
fn f16_stays_rounded_after_arithmetic() {
    // The f32 carrier must not let extra precision survive an operation.
    assert_eq!(run("var f16 (a) = '2048' end set (a) = (a) + '1' end print[(a)] end"), "2048.0");
}

// ------------------------------------------------------- print, chains, escapes

#[test]
fn print_writes_exactly_what_it_is_given() {
    // No separators, and no newline: every line ending is written by hand.
    assert_eq!(run("print[\"1\"] end print[\"2\"] end"), "12");
}

#[test]
fn newlines_are_always_explicit() {
    assert_eq!(run(r#"print["1" \n] end print["2" \n] end"#), "1\n2\n");
    assert_eq!(run(r#"print["1" \n], print["2" \n], print["3" \n] end"#), "1\n2\n3\n");
}

#[test]
fn prints_a_juxtaposed_list() {
    let src = "var str (name) = 'Lua ripoff 🤣' end\nprint[\"Hello, \" (name) \\n] end";
    assert_eq!(run(src), "Hello, Lua ripoff 🤣\n");
}

#[test]
fn an_empty_print_writes_nothing() {
    assert_eq!(run("print[] end"), "");
}

#[test]
fn juxtaposition_stringifies_any_type() {
    let src = "var f16 (x) = '1000' end var u8 (k) = '7' end print[\"x=\" (x) \" k=\" (k)] end";
    assert_eq!(run(src), "x=1000.0 k=7");
}

#[test]
fn bare_escapes_are_text() {
    assert_eq!(run(r#"print["a" \t "b" \n "c"] end"#), "a\tb\nc");
}

#[test]
fn escapes_work_inside_quotes_too() {
    assert_eq!(run("print[\"a\\tb\"] end"), "a\tb");
}

#[test]
fn chains_may_mix_statement_kinds() {
    assert_eq!(run("var i32 (n) = '1', set (n) = (n) + '4', print[(n)] end"), "5");
}

// --------------------------------------------------------------- compile errors

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
    assert!(errors("print[(nope)] end")[0].contains("`(nope)` is not declared"));
}

#[test]
fn suggests_a_near_miss() {
    let d = diags("var i32 (count) = '1' end print[(cont)] end");
    assert!(d[0].help.as_deref().unwrap().contains("(count)"));
}

#[test]
fn rejects_redeclaration() {
    assert!(errors("var i32 (n) = '1' end var i32 (n) = '2' end")[0].contains("already declared"));
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
fn a_lone_statement_still_needs_end() {
    assert!(errors("var i32 (n) = '1'")[0].contains("expected `end`"));
}

#[test]
fn requires_var_after_a_modifier() {
    assert!(errors("global i32 (n) = '1' end")[0].contains("expected `var` after `global`"));
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

#[test]
fn reports_several_errors_at_once() {
    // One bad statement must not hide the ones after it.
    assert_eq!(errors("var u8 (a) = '300' end var u8 (b) = '400' end").len(), 2);
}

#[test]
fn a_declaration_cannot_refer_to_itself() {
    assert!(errors("var i32 (n) = (n) + '1' end")[0].contains("not declared"));
}

// ------------------------------------------------------------ the rules cited

#[test]
fn errors_cite_the_rule_they_broke() {
    let cases: &[(&str, Rule)] = &[
        ("var u8 (n) = '300' end", Rule::ValuesMustFit),
        ("var i32 (n) = '1' end var i64 (m) = (n) end", Rule::NoImplicitConversion),
        ("var number (n) = '1' end", Rule::TypesMustExist),
        ("print[(nope)] end", Rule::NamesMustBeDeclared),
        ("var i32 (n) = '1' end var i32 (n) = '2' end", Rule::NamesAreDeclaredOnce),
        ("var u8 (n) = -'1' end", Rule::UnsignedIsNeverNegative),
        ("var str (a) = 'x' end var str (b) = (a) + 'y' end", Rule::ArithmeticIsNumeric),
        ("var bool (b) = '1' == '2' end", Rule::LiteralsNeedAType),
        ("var i32 (n) = '1'", Rule::EndClosesAChain),
        ("var i32 (n) = '1' end print (n) end", Rule::PrintTakesBrackets),
        ("var i32 (n) = | '1' end", Rule::GroupsArePiped),
        ("var i32 (n) = '1' end var bool (b) = (n) < (n) < (n) end", Rule::ComparisonsDoNotChain),
        (r#"var i32 (n) = \n end"#, Rule::EscapesAreText),
        ("var i32 (n) = '1' end @", Rule::LexicalForm),
        ("global i32 (n) = '1' end", Rule::StatementForm),
        ("var i32 (n = '1' end", Rule::NamesAreParenthesised),
    ];
    for (src, expected) in cases {
        assert_eq!(rule(src), *expected, "wrong rule cited for: {src}");
    }
}

#[test]
fn runtime_errors_cite_rules_too() {
    let e = runtime_error("var u8 (n) = '250' end set (n) = (n) + '10' end");
    assert_eq!(e.rule, Some(Rule::OverflowTraps));

    let e = runtime_error("var u8 (n) = '0' end set (n) = (n) - '1' end");
    assert_eq!(e.rule, Some(Rule::UnsignedIsNeverNegative));

    let e = runtime_error("var i32 (z) = '0' end var i32 (n) = '1' / (z) end");
    assert_eq!(e.rule, Some(Rule::NoDivisionByZero));
}

#[test]
fn a_rendered_error_shows_the_rule() {
    let src = "var u8 (n) = '300' end";
    let text = luarus_diag::render(src, "t.lrs", &diags(src)[0]);
    assert!(text.starts_with("error[values-must-fit]:"), "{text}");
    assert!(text.contains("= rule: a literal must be a valid value"), "{text}");
}

// ------------------------------------------------------------ runtime behaviour

#[test]
fn overflow_traps_instead_of_wrapping() {
    assert!(runtime_error("var u8 (n) = '250' end set (n) = (n) + '10' end")
        .message
        .contains("overflowed `u8`"));
}

#[test]
fn unsigned_subtraction_below_zero_is_an_error() {
    assert!(runtime_error("var u8 (n) = '0' end set (n) = (n) - '1' end")
        .message
        .contains("below zero"));
}

#[test]
fn integer_division_by_zero_is_an_error() {
    assert!(runtime_error("var i32 (z) = '0' end var i32 (n) = '1' / (z) end")
        .message
        .contains("division by zero"));
}

#[test]
fn float_division_by_zero_follows_ieee() {
    assert_eq!(run("var f64 (z) = '0' end var f64 (n) = '1' / (z) end print[(n)] end"), "inf");
}

// --------------------------------------------------------- characters and carets

#[test]
fn a_name_may_be_a_single_multi_scalar_character() {
    assert_eq!(run("var i32 (🧑‍🧑‍🧒‍🧒) = '4' end print[(🧑‍🧑‍🧒‍🧒)] end"), "4");
}

#[test]
fn carets_are_laid_out_in_terminal_cells() {
    // `(🧑‍🧑‍🧒‍🧒)` is three characters but four cells, because the emoji draws
    // double width. The caret is laid out in cells so it lands under the name.
    let src = "var i32 (🧑‍🧑‍🧒‍🧒) = '1' end var i32 (🧑‍🧑‍🧒‍🧒) = '2' end";
    let text = luarus_diag::render(src, "t.lrs", &diags(src)[0]);
    let caret_line = text.lines().find(|l| l.contains('^')).expect("a caret line");
    assert_eq!(caret_line.matches('^').count(), 4, "{text}");
    // ...and the padding before it is 31 cells, not the 30 characters.
    let pad = caret_line.find('^').unwrap() - caret_line.find('|').unwrap() - 2;
    assert_eq!(pad, 31, "{text}");
}

#[test]
fn columns_count_characters_as_a_reader_would() {
    // The column is a character index, not a cell offset: 31 only if the family
    // emoji counted as one character.
    let src = "var i32 (🧑‍🧑‍🧒‍🧒) = '1' end var i32 (🧑‍🧑‍🧒‍🧒) = '2' end";
    let text = luarus_diag::render(src, "t.lrs", &diags(src)[0]);
    assert!(text.contains("t.lrs:1:31"), "{text}");
}
