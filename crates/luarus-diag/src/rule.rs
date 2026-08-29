//! The rules of Luarus, as the compiler and VM name them.
//!
//! Every error identifies the rule it broke. The message says what went wrong
//! here; the rule says what is true everywhere. A language whose selling point
//! is that nothing is implicit ought to be able to state what it is holding you
//! to, and having to name one keeps the rule set honest and small.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    // ---- form
    /// A name is written in parentheses.
    NamesAreParenthesised,
    /// A value is written in quotes.
    ValuesAreQuoted,
    /// A statement chain is closed by `end`.
    EndClosesAChain,
    /// A statement begins with one of the statement keywords.
    StatementForm,
    /// `print` takes its values in brackets.
    PrintTakesBrackets,
    /// An expression is grouped with pipes.
    GroupsArePiped,
    /// A comparison has exactly two sides.
    ComparisonsDoNotChain,
    /// An escape is one of a fixed set, and is always text.
    EscapesAreText,
    /// Every character belongs to a name, a value, an operator or a comment.
    LexicalForm,
    /// A block is delimited by braces.
    BlocksAreBraced,
    /// A condition is a bool; there is no truthiness.
    ConditionsAreBool,
    /// A loop counts over whole numbers.
    LoopsCountWholeNumbers,

    // ---- types
    /// A literal takes its type from its context.
    LiteralsNeedAType,
    /// A literal must be a valid value of the type it is read as.
    ValuesMustFit,
    /// A value never changes type on its own.
    NoImplicitConversion,
    /// A type annotation names one of the built-in types.
    TypesMustExist,
    /// Arithmetic and ordering need numeric types.
    ArithmeticIsNumeric,
    /// An unsigned type never holds a negative value.
    UnsignedIsNeverNegative,

    // ---- names
    /// A name is declared before it is used.
    NamesMustBeDeclared,
    /// A name is declared once.
    NamesAreDeclaredOnce,

    // ---- runtime
    /// Arithmetic never wraps.
    OverflowTraps,
    /// An integer is never divided by zero.
    NoDivisionByZero,
    /// A variable is assigned before it is read.
    AssignBeforeReading,
    /// A chunk must be well formed.
    BytecodeIsWellFormed,
}

impl Rule {
    /// The stable identifier shown in brackets after `error`.
    pub fn slug(self) -> &'static str {
        match self {
            Rule::NamesAreParenthesised => "names-are-parenthesised",
            Rule::ValuesAreQuoted => "values-are-quoted",
            Rule::EndClosesAChain => "end-closes-a-chain",
            Rule::StatementForm => "statement-form",
            Rule::PrintTakesBrackets => "print-takes-brackets",
            Rule::GroupsArePiped => "groups-are-piped",
            Rule::ComparisonsDoNotChain => "comparisons-do-not-chain",
            Rule::EscapesAreText => "escapes-are-text",
            Rule::LexicalForm => "lexical-form",
            Rule::BlocksAreBraced => "blocks-are-braced",
            Rule::ConditionsAreBool => "conditions-are-bool",
            Rule::LoopsCountWholeNumbers => "loops-count-whole-numbers",
            Rule::LiteralsNeedAType => "literals-need-a-type",
            Rule::ValuesMustFit => "values-must-fit",
            Rule::NoImplicitConversion => "no-implicit-conversion",
            Rule::TypesMustExist => "types-must-exist",
            Rule::ArithmeticIsNumeric => "arithmetic-is-numeric",
            Rule::UnsignedIsNeverNegative => "unsigned-is-never-negative",
            Rule::NamesMustBeDeclared => "names-must-be-declared",
            Rule::NamesAreDeclaredOnce => "names-are-declared-once",
            Rule::OverflowTraps => "overflow-traps",
            Rule::NoDivisionByZero => "no-division-by-zero",
            Rule::AssignBeforeReading => "assign-before-reading",
            Rule::BytecodeIsWellFormed => "bytecode-is-well-formed",
        }
    }

    /// The rule itself, stated once, in the same words everywhere.
    pub fn statement(self) -> &'static str {
        match self {
            Rule::NamesAreParenthesised => "a name is written in parentheses, as `(name)`",
            Rule::ValuesAreQuoted => "a value is written in quotes, as `'1000'`",
            Rule::EndClosesAChain => {
                "`end` closes a statement chain, and every chain has one"
            }
            Rule::StatementForm => {
                "a statement starts with `var`, `set`, `print`, `global` or `pub`"
            }
            Rule::PrintTakesBrackets => "`print` takes its values in brackets, as `print[...]`",
            Rule::GroupsArePiped => "an expression is grouped with pipes, as `| ... |`",
            Rule::ComparisonsDoNotChain => "a comparison has exactly two sides",
            Rule::EscapesAreText => {
                "an escape is one of `\\n` `\\t` `\\r` `\\0` `\\\\`, and is always `str`"
            }
            Rule::LexicalForm => {
                "every character belongs to a name, a value, an operator or a comment"
            }
            Rule::BlocksAreBraced => "a block is written `{ ... }`, and `else` divides one in two",
            Rule::ConditionsAreBool => {
                "a condition is a `bool`; no other type is true or false in Luarus"
            }
            Rule::LoopsCountWholeNumbers => {
                "a loop counts whole numbers: an integer type, or an `er` with no fraction"
            }
            Rule::LiteralsNeedAType => {
                "a literal has no type of its own and takes one from its context, or says it \
                 outright as in `f16 '5'`"
            }
            Rule::ValuesMustFit => {
                "a literal must be a valid value of the type it is read as"
            }
            Rule::NoImplicitConversion => "a value never changes type on its own",
            Rule::TypesMustExist => {
                "a type is one of i8 i16 i32 i64, u8 u16 u32 u64, f16 f32 f64, bool, str, nil"
            }
            Rule::ArithmeticIsNumeric => "arithmetic and ordering need numeric types",
            Rule::UnsignedIsNeverNegative => "an unsigned type never holds a negative value",
            Rule::NamesMustBeDeclared => "a name is declared before it is used",
            Rule::NamesAreDeclaredOnce => "a name is declared once",
            Rule::OverflowTraps => "arithmetic never wraps; a result must fit its type",
            Rule::NoDivisionByZero => "an integer is never divided by zero",
            Rule::AssignBeforeReading => "a variable is assigned before it is read",
            Rule::BytecodeIsWellFormed => "a chunk must be well formed to run",
        }
    }

    /// Every rule, for documentation and for `luarus rules`.
    pub const ALL: &'static [Rule] = &[
        Rule::NamesAreParenthesised,
        Rule::ValuesAreQuoted,
        Rule::EndClosesAChain,
        Rule::StatementForm,
        Rule::PrintTakesBrackets,
        Rule::GroupsArePiped,
        Rule::ComparisonsDoNotChain,
        Rule::EscapesAreText,
        Rule::LexicalForm,
        Rule::BlocksAreBraced,
        Rule::ConditionsAreBool,
        Rule::LoopsCountWholeNumbers,
        Rule::LiteralsNeedAType,
        Rule::ValuesMustFit,
        Rule::NoImplicitConversion,
        Rule::TypesMustExist,
        Rule::ArithmeticIsNumeric,
        Rule::UnsignedIsNeverNegative,
        Rule::NamesMustBeDeclared,
        Rule::NamesAreDeclaredOnce,
        Rule::OverflowTraps,
        Rule::NoDivisionByZero,
        Rule::AssignBeforeReading,
        Rule::BytecodeIsWellFormed,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_unique() {
        let mut seen: Vec<&str> = Rule::ALL.iter().map(|r| r.slug()).collect();
        let count = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), count, "two rules share a slug");
    }

    #[test]
    fn every_rule_is_listed_in_all() {
        // ALL is hand-written, so guard against a rule being added without it.
        assert_eq!(Rule::ALL.len(), 24);
    }
}
