//! Position and size expressions, parsed as Marco does at load time (`theme.c:1914-3125`).
//!
//! Evaluating them against a frame is the geometry ticket's job (26). Here they are
//! tokenized, theme constants are substituted, and an expression with no variables is
//! folded to its value, which is what `meta_draw_spec_new` does.

use std::collections::HashMap;

use crate::number::strtod_prefix;

/// A theme's `<constant>`s, which Marco types by their value: a number with a point is a
/// float, another number an integer, and anything else a colour (`theme-parser.c:926`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Constants {
    pub ints: HashMap<String, i32>,
    pub floats: HashMap<String, f64>,
    /// Colour constants hold the colour spec's text. It is parsed where it's used.
    pub colors: HashMap<String, String>,
}

/// A position or size expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i32),
    Double(f64),
    Variable(Variable),
    Binary(Box<Expr>, Operator, Box<Expr>),
    /// An expression Marco loads but can never evaluate, with the error Marco reports
    /// when it tries. Marco draws it as 0.
    Invalid(String),
}

/// The variables Marco defines for expressions (`theme.c:2591`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variable {
    Width,
    Height,
    /// Only defined for `image` and `icon`.
    ObjectWidth,
    /// Only defined for `image` and `icon`.
    ObjectHeight,
    LeftWidth,
    RightWidth,
    TopHeight,
    BottomHeight,
    MiniIconWidth,
    MiniIconHeight,
    IconWidth,
    IconHeight,
    TitleWidth,
    TitleHeight,
    FrameXCenter,
    FrameYCenter,
}

impl Variable {
    fn from_name(name: &str) -> Option<Variable> {
        Some(match name {
            "width" => Variable::Width,
            "height" => Variable::Height,
            "object_width" => Variable::ObjectWidth,
            "object_height" => Variable::ObjectHeight,
            "left_width" => Variable::LeftWidth,
            "right_width" => Variable::RightWidth,
            "top_height" => Variable::TopHeight,
            "bottom_height" => Variable::BottomHeight,
            "mini_icon_width" => Variable::MiniIconWidth,
            "mini_icon_height" => Variable::MiniIconHeight,
            "icon_width" => Variable::IconWidth,
            "icon_height" => Variable::IconHeight,
            "title_width" => Variable::TitleWidth,
            "title_height" => Variable::TitleHeight,
            "frame_x_center" => Variable::FrameXCenter,
            "frame_y_center" => Variable::FrameYCenter,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Mod,
    Max,
    Min,
}

impl Operator {
    fn name(self) -> &'static str {
        match self {
            Operator::Add => "+",
            Operator::Subtract => "-",
            Operator::Multiply => "*",
            Operator::Divide => "/",
            Operator::Mod => "%",
            Operator::Max => "`max`",
            Operator::Min => "`min`",
        }
    }

    /// Marco applies `* / %`, then `+ -`, then `` `max` `` and `` `min` ``.
    fn precedence(self) -> u8 {
        match self {
            Operator::Multiply | Operator::Divide | Operator::Mod => 2,
            Operator::Add | Operator::Subtract => 1,
            Operator::Max | Operator::Min => 0,
        }
    }
}

/// An intermediate value: Marco keeps integers until a float meets them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Value {
    Int(i32),
    Double(f64),
}

impl Value {
    /// Marco's conversion of a result to pixels, which truncates.
    pub(crate) fn to_int(self) -> i32 {
        match self {
            Value::Int(value) => value,
            Value::Double(value) => value as i32,
        }
    }
}

/// Marco's `do_operation`.
pub(crate) fn apply(op: Operator, a: Value, b: Value) -> Result<Value, String> {
    const DIVISION_BY_ZERO: &str = "Coordinate expression results in division by zero";
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(match op {
            Operator::Multiply => a.wrapping_mul(b),
            Operator::Divide if b == 0 => return Err(DIVISION_BY_ZERO.to_owned()),
            Operator::Divide => a.wrapping_div(b),
            Operator::Mod if b == 0 => return Err(DIVISION_BY_ZERO.to_owned()),
            Operator::Mod => a.wrapping_rem(b),
            Operator::Add => a.wrapping_add(b),
            Operator::Subtract => a.wrapping_sub(b),
            Operator::Max => a.max(b),
            Operator::Min => a.min(b),
        })),
        (a, b) => {
            let a = as_double(a);
            let b = as_double(b);
            Ok(Value::Double(match op {
                Operator::Multiply => a * b,
                Operator::Divide if b == 0.0 => return Err(DIVISION_BY_ZERO.to_owned()),
                Operator::Divide => a / b,
                Operator::Mod => {
                    return Err("Coordinate expression tries to use mod operator on a \
                                floating-point number"
                        .to_owned());
                }
                Operator::Add => a + b,
                Operator::Subtract => a - b,
                // C's MAX and MIN.
                Operator::Max => {
                    if a > b {
                        a
                    } else {
                        b
                    }
                }
                Operator::Min => {
                    if a < b {
                        a
                    } else {
                        b
                    }
                }
            }))
        }
    }
}

fn as_double(value: Value) -> f64 {
    match value {
        Value::Int(value) => f64::from(value),
        Value::Double(value) => value,
    }
}

#[derive(Debug, Clone)]
enum Token {
    Int(i32),
    Double(f64),
    Operator(Operator),
    Name(String),
    Open,
    Close,
}

/// An operand or operator at one parenthesis level.
enum Item {
    Operand(Expr),
    Operator(Operator),
}

/// Marco's hard limit on the terms at one parenthesis level.
const MAX_EXPRS: usize = 32;

impl Expr {
    /// Parses an expression as Marco does when it loads a theme. It never fails: Marco
    /// loads malformed expressions too, so those become [`Expr::Invalid`].
    pub fn parse(text: &str, constants: &Constants) -> Expr {
        let tokens = match tokenize(text) {
            Ok(tokens) => tokens,
            Err(message) => return Expr::Invalid(message),
        };
        let mut constant = true;
        let tokens: Vec<Token> = tokens
            .into_iter()
            .map(|token| match token {
                Token::Name(name) => {
                    if let Some(&value) = constants.ints.get(&name) {
                        Token::Int(value)
                    } else if let Some(&value) = constants.floats.get(&name) {
                        Token::Double(value)
                    } else {
                        constant = false;
                        Token::Name(name)
                    }
                }
                token => token,
            })
            .collect();
        let expr = match build(&tokens) {
            Ok(expr) => expr,
            Err(message) => return Expr::Invalid(message),
        };
        if !constant {
            return expr;
        }
        match fold(&expr) {
            Ok(value) => Expr::Int(value.to_int()),
            Err(message) => Expr::Invalid(message),
        }
    }
}

fn fold(expr: &Expr) -> Result<Value, String> {
    match expr {
        Expr::Int(value) => Ok(Value::Int(*value)),
        Expr::Double(value) => Ok(Value::Double(*value)),
        Expr::Binary(a, op, b) => apply(*op, fold(a)?, fold(b)?),
        Expr::Variable(_) | Expr::Invalid(_) => {
            unreachable!("a constant expression has no variables")
        }
    }
}

/// Marco's `pos_tokenize`.
fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'*' | b'/' | b'+' | b'-' | b'%' | b'`' => {
                let rest = &bytes[i..];
                let (op, len) = match c {
                    b'*' => (Operator::Multiply, 1),
                    b'/' => (Operator::Divide, 1),
                    b'+' => (Operator::Add, 1),
                    b'-' => (Operator::Subtract, 1),
                    b'%' => (Operator::Mod, 1),
                    _ if rest.starts_with(b"`max`") => (Operator::Max, 5),
                    _ if rest.starts_with(b"`min`") => (Operator::Min, 5),
                    _ => {
                        return Err(format!(
                            "Coordinate expression contained unknown operator at the start of \
                             this text: \"{}\"",
                            &text[i..]
                        ));
                    }
                };
                tokens.push(Token::Operator(op));
                i += len;
            }
            b'(' => {
                tokens.push(Token::Open);
                i += 1;
            }
            b')' => {
                tokens.push(Token::Close);
                i += 1;
            }
            b' ' | b'\t' | b'\n' => i += 1,
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
                    i += 1;
                }
                tokens.push(Token::Name(text[start..i].to_owned()));
            }
            _ => {
                let start = i;
                while i < bytes.len() && (bytes[i] == b'.' || bytes[i].is_ascii_digit()) {
                    i += 1;
                }
                let number = &text[start..i];
                if number.is_empty() {
                    let c = text[start..].chars().next().unwrap_or_default();
                    return Err(format!(
                        "Coordinate expression contains character '{c}' which is not allowed"
                    ));
                }
                if number.contains('.') {
                    match strtod_prefix(number) {
                        Some((value, _)) => tokens.push(Token::Double(value)),
                        None => {
                            return Err(format!(
                                "Coordinate expression contains floating point number '{number}' \
                                 which could not be parsed"
                            ));
                        }
                    }
                } else {
                    // strtol saturates, and Marco stores the long in an int.
                    let value = number.parse::<i64>().unwrap_or(i64::MAX);
                    tokens.push(Token::Int(value as i32));
                }
            }
        }
    }
    if tokens.is_empty() {
        return Err("Coordinate expression was empty or not understood".to_owned());
    }
    Ok(tokens)
}

/// Marco's `pos_eval_helper`, building a tree where Marco computes a value.
fn build(tokens: &[Token]) -> Result<Expr, String> {
    let mut items: Vec<Item> = Vec::new();
    let mut level = 0;
    let mut first_paren = 0;
    for (i, token) in tokens.iter().enumerate() {
        if items.len() >= MAX_EXPRS {
            return Err("Coordinate expression parser overflowed its buffer.".to_owned());
        }
        if level == 0 {
            match token {
                Token::Int(value) => items.push(Item::Operand(Expr::Int(*value))),
                Token::Double(value) => items.push(Item::Operand(Expr::Double(*value))),
                Token::Open => {
                    level += 1;
                    first_paren = i;
                }
                Token::Close => {
                    return Err(
                        "Coordinate expression had a close parenthesis with no open \
                                parenthesis"
                            .to_owned(),
                    );
                }
                Token::Name(name) => match Variable::from_name(name) {
                    Some(variable) => items.push(Item::Operand(Expr::Variable(variable))),
                    None => {
                        return Err(format!(
                            "Coordinate expression had unknown variable or constant \"{name}\""
                        ));
                    }
                },
                Token::Operator(op) => items.push(Item::Operator(*op)),
            }
        } else {
            match token {
                Token::Open => level += 1,
                Token::Close => {
                    if level == 1 {
                        items.push(Item::Operand(build(&tokens[first_paren + 1..i])?));
                    }
                    level -= 1;
                }
                _ => {}
            }
        }
    }
    if level > 0 {
        return Err(
            "Coordinate expression had an open parenthesis with no close parenthesis".to_owned(),
        );
    }
    if items.is_empty() {
        return Err(
            "Coordinate expression doesn't seem to have any operators or operands".to_owned(),
        );
    }
    for precedence in [2, 1, 0] {
        combine(&mut items, precedence)?;
    }
    match items.pop() {
        Some(Item::Operand(expr)) if items.is_empty() => Ok(expr),
        _ => unreachable!("combining leaves one operand"),
    }
}

/// Marco's `do_operations`: one left-to-right pass that combines the operators of one
/// precedence, checking that operands and operators alternate.
fn combine(items: &mut Vec<Item>, precedence: u8) -> Result<(), String> {
    let mut i = 1;
    while i < items.len() {
        if let Item::Operator(op) = items[i - 1] {
            return Err(format!(
                "Coordinate expression has an operator \"{}\" where an operand was expected",
                op.name()
            ));
        }
        let Item::Operator(op) = items[i] else {
            return Err(
                "Coordinate expression had an operand where an operator was expected".to_owned(),
            );
        };
        if i == items.len() - 1 {
            return Err(
                "Coordinate expression ended with an operator instead of an operand".to_owned(),
            );
        }
        if let Item::Operator(next) = items[i + 1] {
            return Err(format!(
                "Coordinate expression has operator \"{}\" following operator \"{}\" with no \
                 operand in between",
                next.name(),
                op.name()
            ));
        }
        if op.precedence() == precedence {
            let Item::Operand(b) = items.remove(i + 1) else {
                unreachable!()
            };
            items.remove(i);
            let Item::Operand(a) = std::mem::replace(&mut items[i - 1], Item::Operator(op)) else {
                unreachable!()
            };
            items[i - 1] = Item::Operand(Expr::Binary(Box::new(a), op, Box::new(b)));
        } else {
            i += 2;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constants() -> Constants {
        let mut constants = Constants::default();
        constants.ints.insert("ButtonWidth".to_owned(), 18);
        constants.floats.insert("Half".to_owned(), 0.5);
        constants
    }

    fn parse(text: &str) -> Expr {
        Expr::parse(text, &constants())
    }

    fn var(variable: Variable) -> Box<Expr> {
        Box::new(Expr::Variable(variable))
    }

    fn int(value: i32) -> Box<Expr> {
        Box::new(Expr::Int(value))
    }

    fn binary(left: Box<Expr>, op: Operator, right: Box<Expr>) -> Expr {
        Expr::Binary(left, op, right)
    }

    #[test]
    fn a_variable_is_kept_for_evaluation() {
        assert_eq!(parse("width"), Expr::Variable(Variable::Width));
        assert_eq!(
            parse("mini_icon_height"),
            Expr::Variable(Variable::MiniIconHeight)
        );
        assert_eq!(
            parse("frame_x_center"),
            Expr::Variable(Variable::FrameXCenter)
        );
    }

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        assert_eq!(
            parse("1 + width * 2"),
            binary(
                int(1),
                Operator::Add,
                Box::new(binary(var(Variable::Width), Operator::Multiply, int(2)))
            )
        );
    }

    #[test]
    fn max_and_min_bind_loosest() {
        assert_eq!(
            parse("height `max` width + 1"),
            binary(
                var(Variable::Height),
                Operator::Max,
                Box::new(binary(var(Variable::Width), Operator::Add, int(1)))
            )
        );
        assert_eq!(
            parse("height`min`width"),
            binary(var(Variable::Height), Operator::Min, var(Variable::Width))
        );
    }

    #[test]
    fn operators_of_equal_precedence_associate_left() {
        assert_eq!(
            parse("width - 2 - 1"),
            binary(
                Box::new(binary(var(Variable::Width), Operator::Subtract, int(2))),
                Operator::Subtract,
                int(1)
            )
        );
        assert_eq!(
            parse("width / 2 % 3"),
            binary(
                Box::new(binary(var(Variable::Width), Operator::Divide, int(2))),
                Operator::Mod,
                int(3)
            )
        );
    }

    #[test]
    fn parentheses_group() {
        assert_eq!(
            parse("(width - 2) * 3"),
            binary(
                Box::new(binary(var(Variable::Width), Operator::Subtract, int(2))),
                Operator::Multiply,
                int(3)
            )
        );
    }

    #[test]
    fn constants_are_substituted() {
        assert_eq!(
            parse("width - ButtonWidth"),
            binary(var(Variable::Width), Operator::Subtract, int(18))
        );
        assert_eq!(
            parse("width * Half"),
            binary(
                var(Variable::Width),
                Operator::Multiply,
                Box::new(Expr::Double(0.5))
            )
        );
    }

    #[test]
    fn an_expression_without_variables_is_folded_and_truncated() {
        assert_eq!(parse("(10 + 4) / 3"), Expr::Int(4));
        assert_eq!(parse("7 % 4"), Expr::Int(3));
        assert_eq!(parse("7.0 / 2"), Expr::Int(3));
        assert_eq!(parse("ButtonWidth * Half + 0.9"), Expr::Int(9));
        assert_eq!(parse("3 `max` 5 `min` 4"), Expr::Int(4));
    }

    #[test]
    fn a_float_is_a_number_with_a_point() {
        assert_eq!(
            parse("width * .5"),
            binary(
                var(Variable::Width),
                Operator::Multiply,
                Box::new(Expr::Double(0.5))
            )
        );
    }

    fn invalid(text: &str) -> String {
        match parse(text) {
            Expr::Invalid(message) => message,
            other => panic!("{text:?} parsed as {other:?}"),
        }
    }

    // Marco loads a theme with any of these, and then fails to evaluate them.
    #[test]
    fn a_malformed_expression_loads_as_invalid() {
        assert!(invalid("").contains("empty"));
        assert!(invalid("width $ 2").contains("'$'"));
        assert!(invalid("width `mid` 2").contains("unknown operator"));
        assert!(invalid("-1").contains("where an operand was expected"));
        assert!(invalid("width +").contains("ended with an operator"));
        assert!(invalid("width 2").contains("operand where an operator"));
        assert!(invalid("width * / 2").contains("following operator"));
        assert!(invalid("(width").contains("open parenthesis"));
        assert!(invalid("width)").contains("close parenthesis"));
        assert!(invalid("()").contains("any operators or operands"));
        assert!(invalid("Undefined + 1").contains("\"Undefined\""));
        assert!(invalid("4 / (2 - 2)").contains("division by zero"));
        assert!(invalid("5 % 2.0").contains("mod operator"));
    }

    #[test]
    fn more_than_32_terms_at_one_level_is_invalid() {
        let sixteen = vec!["1"; 16].join(" + ");
        assert_eq!(parse(&sixteen), Expr::Int(16));
        assert!(invalid(&format!("{sixteen} + 1")).contains("overflowed"));
        // Parentheses start a new count.
        assert_eq!(parse(&format!("({sixteen}) + ({sixteen})")), Expr::Int(32));
    }
}
