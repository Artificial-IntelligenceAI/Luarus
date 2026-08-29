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

// ------------------------------------------------------------ if and else

#[test]
fn runs_the_worked_if_example() {
    let src = "var f16 (x) = '1000' end\n\
               if (x) > f16 '5' {\n\
               print[\"x is greater than 5\" \\n] end\n\
               else print[\"x is less than 5\" \\n] end }";
    assert_eq!(run(src), "x is greater than 5\n");
}

#[test]
fn takes_the_else_arm_when_the_condition_is_false() {
    let src = "var f16 (x) = '1' end if (x) > f16 '5' { print[\"greater\"] end \
               else print[\"less\"] end }";
    assert_eq!(run(src), "less");
}

#[test]
fn an_if_needs_no_else() {
    assert_eq!(run("var i32 (n) = '7' end if (n) > '5' { print[\"big\"] end }"), "big");
    assert_eq!(run("var i32 (n) = '1' end if (n) > '5' { print[\"big\"] end } print[\".\"] end"), ".");
}

#[test]
fn ifs_nest() {
    let src = "var i32 (n) = '7' end \
               if (n) > '5' { print[\"big \"] end \
                 if (n) > '6' { print[\"very\"] end else print[\"not huge\"] end } } \
               print[\" done\"] end";
    assert_eq!(run(src), "big very done");
}

#[test]
fn elseif_chains_inside_one_brace_pair() {
    let grade = |n: &str| {
        run(&format!(
            "var i32 (n) = '{n}' end \
             if (n) > '90' {{ print[\"a\"] end \
             elseif (n) > '80' print[\"b\"] end \
             elseif (n) > '70' print[\"c\"] end \
             else print[\"f\"] end }}"
        ))
    };
    assert_eq!(grade("95"), "a");
    assert_eq!(grade("85"), "b");
    assert_eq!(grade("75"), "c");
    assert_eq!(grade("20"), "f");
}

#[test]
fn a_chain_of_any_length_closes_with_one_brace() {
    // The whole point of `elseif`: braces do not pile up.
    let src = "var i32 (n) = '5' end \
               if (n) > '9' { print[\"a\"] end \
               elseif (n) > '8' print[\"b\"] end \
               elseif (n) > '7' print[\"c\"] end \
               elseif (n) > '4' print[\"d\"] end \
               else print[\"e\"] end }";
    assert_eq!(src.matches('{').count(), 1);
    assert_eq!(src.matches('}').count(), 1);
    assert_eq!(run(src), "d");
}

#[test]
fn elseif_needs_no_else() {
    let src = "var i32 (n) = '1' end \
               if (n) > '9' { print[\"a\"] end elseif (n) > '8' print[\"b\"] end } \
               print[\"through\"] end";
    assert_eq!(run(src), "through");
}

#[test]
fn else_may_still_hold_another_if() {
    // Nesting kept working; `elseif` is the tidier way to say the same thing.
    let src = "var i32 (n) = '85' end \
               if (n) > '90' { print[\"a\"] end \
               else if (n) > '80' { print[\"b\"] end else print[\"c\"] end } }";
    assert_eq!(run(src), "b");
}

#[test]
fn elseif_may_not_follow_else() {
    let d = diags(
        "var i32 (n) = '1' end if (n) > '9' { print[\"a\"] end else print[\"b\"] end \
         elseif (n) > '5' print[\"c\"] end }",
    );
    assert_eq!(d[0].rule, Rule::BlocksAreBraced);
    assert!(d[0].message.contains("cannot come after `else`"), "{:?}", d[0].message);
}

#[test]
fn a_block_may_have_only_one_else() {
    let d = diags(
        "var i32 (n) = '1' end if (n) > '9' { print[\"a\"] end else print[\"b\"] end \
         else print[\"c\"] end }",
    );
    assert!(d[0].message.contains("only one `else`"), "{:?}", d[0].message);
}

#[test]
fn a_stray_closing_brace_terminates() {
    // Recovery stops at `}` rather than consuming it, so without a progress
    // guard the parser spun forever on one at the top level.
    let d = diags("print[\"hi\"] end }");
    assert_eq!(d[0].rule, Rule::BlocksAreBraced);
    assert!(d[0].message.contains("unmatched"), "{:?}", d[0].message);
}

#[test]
fn every_arm_of_a_chain_is_checked() {
    let e = errors(
        "var i32 (n) = '1' end if (n) > '9' { print[\"a\"] end \
         elseif (n) > '5' print[(gone)] end }",
    );
    assert!(e[0].contains("`(gone)` is not declared"), "{e:?}");
}

#[test]
fn both_arms_are_type_checked_even_though_one_runs() {
    let e = errors("var i32 (n) = '1' end if (n) > '0' { print[\"ok\"] end else print[(gone)] end }");
    assert!(e[0].contains("`(gone)` is not declared"), "{e:?}");
}

#[test]
fn a_block_is_a_scope() {
    let e = errors(
        "var i32 (n) = '1' end if (n) > '0' { var i32 (inner) = '5' end } print[(inner)] end",
    );
    assert!(e[0].contains("`(inner)` is not declared"), "{e:?}");
}

#[test]
fn a_block_can_read_the_scope_around_it() {
    assert_eq!(run("var i32 (n) = '7' end if (n) > '5' { print[(n)] end }"), "7");
}

#[test]
fn there_is_no_truthiness() {
    let d = diags("var i32 (n) = '1' end if (n) { print[\"x\"] end }");
    assert_eq!(d[0].rule, Rule::ConditionsAreBool);
    assert!(d[0].message.contains("`i32`"), "{:?}", d[0].message);
}

#[test]
fn an_unterminated_block_is_reported() {
    let d = diags("var i32 (n) = '1' end if (n) > '0' { print[\"hi\"] end");
    assert_eq!(d[0].rule, Rule::BlocksAreBraced);
}

#[test]
fn a_condition_may_be_a_bare_bool() {
    assert_eq!(run("var bool (go) = 'true' end if (go) { print[\"yes\"] end }"), "yes");
}

// ------------------------------------------------------- typed literals

#[test]
fn a_typed_literal_supplies_its_own_type() {
    // Each of these would be "cannot tell what type this value is" unadorned.
    assert_eq!(run("print[i32 '42' \" \" f64 '1.5' \" \" str '12'] end"), "42 1.5 12");
}

#[test]
fn a_typed_literal_works_where_context_already_gives_one() {
    assert_eq!(run("var f16 (x) = f16 '1000' end print[(x)] end"), "1000.0");
}

#[test]
fn a_typed_literal_may_not_disagree_with_its_context() {
    let d = diags("var i32 (n) = f64 '1' end");
    assert_eq!(d[0].rule, Rule::NoImplicitConversion);
    assert!(d[0].message.contains("this literal says `f64`"), "{:?}", d[0].message);
}

#[test]
fn a_typed_literal_is_still_range_checked() {
    let d = diags("print[u8 '300'] end");
    assert_eq!(d[0].rule, Rule::ValuesMustFit);
}

#[test]
fn a_typed_literal_needs_a_real_type() {
    assert_eq!(diags("print[number '1'] end")[0].rule, Rule::TypesMustExist);
}

#[test]
fn a_type_word_must_be_followed_by_a_literal() {
    assert_eq!(diags("print[i32 (x)] end")[0].rule, Rule::LiteralsNeedAType);
}

#[test]
fn a_comparison_of_two_typed_literals_needs_no_context() {
    // This is what `literals-need-a-type` used to reject outright.
    assert_eq!(run("var bool (b) = i32 '1' == i32 '2' end print[(b)] end"), "false");
}

// ------------------------------------------------------------------- loops

#[test]
fn runs_the_worked_loop_example() {
    assert_eq!(run("loop perm store-in i32 (i) = '0' to '10' end print[(i)] end"), "10");
}

#[test]
fn a_loop_is_inclusive_at_both_ends() {
    // Eleven values, 0 through 10, so the target ends on 10 rather than 9 or 11.
    assert_eq!(run("loop perm store-in i32 (i) = '0' to '10' end print[(i)] end"), "10");
    assert_eq!(run("loop perm store-in i32 (i) = '5' to '5' end print[(i)] end"), "5");
}

#[test]
fn a_loop_reaches_the_top_of_its_type_without_overflowing() {
    // The step happens only while the counter is strictly below the bound, so
    // counting up to the maximum never computes maximum + 1.
    assert_eq!(run("loop perm store-in u8 (i) = '250' to '255' end print[(i)] end"), "255");
    assert_eq!(run("loop perm store-in i8 (i) = '-128' to '127' end print[(i)] end"), "127");
    assert_eq!(run("loop perm store-in i8 (i) = '0' to '127' end print[(i)] end"), "127");
}

#[test]
fn an_empty_range_stores_nothing() {
    // Counting down is empty rather than reversed, so the target is never
    // assigned and reading it says so.
    let e = runtime_error("loop perm store-in i32 (i) = '10' to '0' end print[(i)] end");
    assert_eq!(e.rule, Some(Rule::AssignBeforeReading));
}

#[test]
fn perm_is_what_keeps_the_target_alive() {
    assert!(errors("loop store-in i32 (i) = '0' to '10' end print[(i)] end")[0]
        .contains("`(i)` is not declared"));
}

#[test]
fn loop_bounds_are_ordinary_expressions() {
    let src = "var i32 (n) = '3' end \
               loop perm store-in i32 (i) = (n) to | (n) * '2' | end print[(i)] end";
    assert_eq!(run(src), "6");
}

#[test]
fn a_loop_counts_only_over_integers() {
    let d = diags("loop perm store-in f64 (i) = '0' to '10' end");
    assert_eq!(d[0].rule, Rule::LoopsCountWholeNumbers);
    assert!(d[0].message.contains("`f64`"), "{:?}", d[0].message);
    assert_eq!(diags("loop perm store-in str (i) = '0' to '10' end")[0].rule, Rule::LoopsCountWholeNumbers);
}

#[test]
fn a_loop_cannot_count_from_itself() {
    // The bounds are checked before the name is bound.
    assert!(errors("loop perm store-in i32 (i) = (i) to '10' end")[0].contains("not declared"));
}

#[test]
fn a_loop_target_may_not_shadow_a_visible_name() {
    assert!(errors("var i32 (n) = '1' end loop perm store-in i32 (n) = '0' to '5' end")[0]
        .contains("already declared"));
}

#[test]
fn store_in_is_one_keyword_and_a_hyphen_is_still_a_minus() {
    // The hyphen joins two letters, so subtraction and negation are untouched.
    assert_eq!(run("var i32 (a) = '7'-'3' end print[(a)] end"), "4");
    assert_eq!(run("var i32 (b) = -'3' end print[(b)] end"), "-3");
    // `store-in` is optional, so leaving it out is only wrong once a type
    // appears where the `=` should be.
    let d = diags("loop perm i32 (i) = '0' to '3' end");
    assert!(d[0].message.contains("expected `=`"), "{:?}", d[0].message);
    assert!(d[0].help.as_deref().unwrap().contains("store-in"));
}

#[test]
fn a_missing_to_is_reported() {
    assert!(errors("loop perm store-in i32 (i) = '0' '3' end")[0].contains("expected `to`"));
}

// -------------------------------------------------------- loops with bodies

#[test]
fn runs_the_worked_loop_body_example() {
    let out = run("loop temp = '0' to '10' {\nprint[\"Hello\" \\n] end }");
    assert_eq!(out.lines().count(), 11);
    assert!(out.lines().all(|l| l == "Hello"));
}

#[test]
fn a_body_runs_once_per_value() {
    assert_eq!(run("loop temp store-in i32 (i) = '0' to '3' { print[(i)] end }"), "0123");
}

#[test]
fn temp_confines_the_target_to_the_body() {
    assert_eq!(run("loop temp store-in i32 (i) = '1' to '3' { print[(i)] end }"), "123");
    assert!(errors("loop temp store-in i32 (i) = '1' to '3' { print[(i)] end } print[(i)] end")[0]
        .contains("`(i)` is not declared"));
}

#[test]
fn perm_keeps_the_target_and_the_body_can_still_see_it() {
    let src = "loop perm store-in i32 (i) = '1' to '3' { print[(i)] end } print[\"|\" (i)] end";
    assert_eq!(run(src), "123|3");
}

#[test]
fn a_loop_with_no_target_just_repeats() {
    assert_eq!(run("loop temp = '1' to '4' { print[\"x\"] end }"), "xxxx");
    // `temp` is the default said out loud, so leaving it off means the same.
    assert_eq!(run("loop = '1' to '4' { print[\"x\"] end }"), "xxxx");
}

#[test]
fn loops_nest() {
    let src = "loop temp store-in i32 (a) = '1' to '3' { \
                 loop temp store-in i32 (b) = '1' to '2' { print[(a) (b) \" \"] end } }";
    assert_eq!(run(src), "11 12 21 22 31 32 ");
}

#[test]
fn a_loop_body_holds_whatever_a_block_holds() {
    let src = "loop temp store-in i32 (n) = '1' to '4' { \
                 if (n) > '2' { print[\"b\"] end else print[\"s\"] end } }";
    assert_eq!(run(src), "ssbb");
}

#[test]
fn a_body_over_an_empty_range_never_runs() {
    assert_eq!(run("loop temp = '5' to '1' { print[\"never\"] end } print[\"done\"] end"), "done");
}

#[test]
fn untargeted_bounds_fall_back_to_their_own_type() {
    // Nothing declares a type, so the bounds keep i64 and the loop still counts.
    assert_eq!(run("loop temp = '1' to '3' { print[\".\"] end }"), "...");
    // A typed bound is used in preference to the fallback.
    assert_eq!(run("loop temp = u8 '1' to u8 '3' { print[\".\"] end }"), "...");
}

#[test]
fn a_loop_body_is_a_scope() {
    let src = "loop temp = '1' to '2' { var i32 (inner) = '1' end } print[(inner)] end";
    assert!(errors(src)[0].contains("`(inner)` is not declared"));
}

#[test]
fn an_unclosed_loop_body_is_reported() {
    let d = diags("loop temp = '1' to '2' { print[\"x\"] end");
    assert_eq!(d[0].rule, Rule::BlocksAreBraced);
}

// ------------------------------------------------------ exact rationals (er)

#[test]
fn er_is_exact_where_a_float_is_not() {
    // The canonical float embarrassment, on both types side by side.
    assert_eq!(run("var f64 (a) = '0.1' end print[|(a) + '0.2'|] end"), "0.30000000000000004");
    assert_eq!(run("var er (a) = '0.1' end print[|(a) + '0.2'|] end"), "0.3");
    assert_eq!(run("var f64 (a) = '0.1' end print[||(a) + '0.2'| == '0.3'|] end"), "false");
    assert_eq!(run("var er (a) = '0.1' end print[||(a) + '0.2'| == '0.3'|] end"), "true");
}

#[test]
fn er_division_is_exact_and_closed() {
    assert_eq!(run("print[|er '1' / er '3'|] end"), "1/3");
    assert_eq!(run("print[||er '1' / er '3'| * er '3'|] end"), "1");
    assert_eq!(run("print[|er '1/3' + er '1/6'|] end"), "0.5");
}

#[test]
fn er_prints_a_decimal_when_one_terminates_and_a_fraction_otherwise() {
    assert_eq!(run("print[er '1/2' \" \" er '1/8' \" \" er '1/10'] end"), "0.5 0.125 0.1");
    assert_eq!(run("print[er '1/3' \" \" er '22/7'] end"), "1/3 22/7");
    // Always in lowest terms, so these are the same value.
    assert_eq!(run("print[|er '2/4' == er '1/2'|] end"), "true");
    assert_eq!(run("print[er '6/3'] end"), "2");
}

#[test]
fn er_is_unbounded() {
    let src = "var er (v) = '1' end loop temp = '80' times { set (v) = (v) * '3' end } print[(v)] end";
    assert_eq!(run(src), "147808829414345923316083210206383297601");
}

#[test]
fn er_does_not_accumulate_error() {
    let src = "var er (s) = '0' end loop temp = '1000' times { set (s) = (s) + er '0.1' end } \
               print[(s)] end";
    assert_eq!(run(src), "100");
}

#[test]
fn er_orders_across_signs_and_denominators() {
    assert_eq!(run("print[|er '1/3' < er '1/2'|] end"), "true");
    assert_eq!(run("print[|er '-1/3' < er '-1/2'|] end"), "false");
    assert_eq!(run("print[|er '-1' < er '0'|] end"), "true");
}

#[test]
fn er_never_overflows_but_still_refuses_a_zero_divisor() {
    let e = runtime_error("var er (z) = '0' end print[|er '1' / (z)|] end");
    assert_eq!(e.rule, Some(Rule::NoDivisionByZero));
}

#[test]
fn er_takes_integers_decimals_and_fractions() {
    assert_eq!(run("print[er '3' \" \" er '-2.25' \" \" er '1_000.5' \" \" er '-2/7'] end"),
               "3 -2.25 1000.5 -2/7");
    assert_eq!(diags("print[er 'x'] end")[0].rule, Rule::ValuesMustFit);
    // A zero denominator is not a rational at all, so it fails at compile time.
    assert_eq!(diags("print[er '1/0'] end")[0].rule, Rule::ValuesMustFit);
}

#[test]
fn er_does_not_mix_with_the_other_numeric_types() {
    assert!(errors("var f64 (a) = '1' end var er (b) = (a) end")[0].contains("expected `er`"));
    assert!(errors("var er (a) = '1' end var i32 (b) = (a) end")[0].contains("expected `i32`"));
}

#[test]
fn a_loop_may_count_over_er() {
    // An exact rational steps by one as exactly as any integer, and being
    // unbounded it can count further than any of them.
    assert_eq!(run("loop temp store-in er (i) = er '0' to er '5' { print[(i)] end }"), "012345");
    assert_eq!(run("loop temp = er '11' times { print[\"x\"] end }").len(), 11);
}

#[test]
fn an_er_loop_bound_must_be_whole() {
    // Written down, the checker sees it.
    let d = diags("loop temp = er '1/3' times { print[\"x\"] end }");
    assert_eq!(d[0].rule, Rule::LoopsCountWholeNumbers);
    assert!(d[0].message.contains("1/3"), "{:?}", d[0].message);

    // Computed, only the VM can.
    let e = runtime_error(
        "var er (a) = '10/3' end loop temp store-in er (i) = er '0' to (a) { print[\"x\"] end }",
    );
    assert_eq!(e.rule, Some(Rule::LoopsCountWholeNumbers));
}

#[test]
fn an_er_loop_counts_past_every_fixed_width() {
    // 2^200 by doubling: no other numeric type in the language reaches it.
    let src = "var er (n) = '1' end loop temp = '200' times { set (n) = (n) * '2' end } print[(n)] end";
    assert_eq!(run(src).len(), 61);
}

#[test]
fn floats_still_cannot_be_counted() {
    assert_eq!(diags("loop temp = f64 '3' times {}")[0].rule, Rule::LoopsCountWholeNumbers);
    assert_eq!(diags("loop temp store-in str (i) = '0' to '3' end")[0].rule, Rule::LoopsCountWholeNumbers);
}

// ------------------------------------------------------------------- times

#[test]
fn runs_the_worked_times_example() {
    let out = run("loop temp = '11' times {\nprint[\"Hello\" \\n] end }");
    assert_eq!(out.lines().count(), 11);
}

#[test]
fn times_counts_from_zero_and_stops_below_the_count() {
    assert_eq!(run("loop temp store-in i32 (i) = '5' times { print[(i)] end }"), "01234");
    // Which makes it the `to` form shifted by one.
    assert_eq!(
        run("loop temp store-in i32 (i) = '5' times { print[(i)] end }"),
        run("loop temp store-in i32 (j) = '0' to '4' { print[(j)] end }")
    );
}

#[test]
fn zero_times_runs_nothing() {
    assert_eq!(run("loop temp = '0' times { print[\"never\"] end } print[\"done\"] end"), "done");
}

#[test]
fn a_count_needs_no_type_and_may_not_be_negative() {
    // `times` is the annotation: a count falls back to u64, which has no
    // negative values to offer.
    assert_eq!(run("loop temp = '3' times { print[\".\"] end }"), "...");
    let d = diags("loop temp = '-1' times { print[\".\"] end }");
    assert_eq!(d[0].rule, Rule::ValuesMustFit);
    assert!(d[0].help.as_deref().unwrap().contains("unsigned"));
}

#[test]
fn times_reaches_the_top_of_its_type() {
    // 255 values on a u8 is 0 through 254, and the final step to 255 fits.
    assert_eq!(run("loop perm store-in u8 (i) = '255' times {} print[(i)] end"), "254");
    assert_eq!(diags("loop temp store-in u8 (i) = '256' times {}")[0].rule, Rule::ValuesMustFit);
}

#[test]
fn a_loop_wants_to_or_times() {
    let d = diags("loop temp = '3' end");
    assert!(d[0].message.contains("expected `to` or `times`"), "{:?}", d[0].message);
}
