// SPDX-FileCopyrightText: 2001 Havoc Pennington
// SPDX-FileCopyrightText: 2025-2026 Juan Pablo Lassala
// SPDX-License-Identifier: GPL-3.0-or-later AND GPL-2.0-or-later

//! Position and size expressions, parsed as Marco does at load time (`theme.c:1914-3125`).
//!
//! When a theme loads they are tokenized, theme constants are substituted, and an
//! expression with no variables is folded to its value, which is what
//! `meta_draw_spec_new` does. Drawing evaluates the rest against an [`ExprEnv`].

use std::collections::HashMap;

use crate::geometry::Rect;
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

/// The values of an expression's variables where it's drawn (Marco's
/// `MetaPositionExprEnv`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExprEnv {
    /// The rectangle being drawn into: positions are offset by its origin, and it
    /// gives `width` and `height`.
    pub rect: Rect,
    /// `object_width`, which Marco defines only for `image` and `icon`.
    pub object_width: Option<i32>,
    /// `object_height`, which Marco defines only for `image` and `icon`.
    pub object_height: Option<i32>,
    pub left_width: i32,
    pub right_width: i32,
    pub top_height: i32,
    pub bottom_height: i32,
    pub mini_icon_width: i32,
    pub mini_icon_height: i32,
    pub icon_width: i32,
    pub icon_height: i32,
    pub title_width: i32,
    pub title_height: i32,
    pub frame_x_center: i32,
    pub frame_y_center: i32,
}

impl ExprEnv {
    /// Marco's `pos_eval_get_variable`.
    fn get(&self, variable: Variable) -> Result<i32, String> {
        Ok(match variable {
            Variable::Width => self.rect.width,
            Variable::Height => self.rect.height,
            Variable::ObjectWidth => self
                .object_width
                .ok_or_else(|| unknown_variable("object_width"))?,
            Variable::ObjectHeight => self
                .object_height
                .ok_or_else(|| unknown_variable("object_height"))?,
            Variable::LeftWidth => self.left_width,
            Variable::RightWidth => self.right_width,
            Variable::TopHeight => self.top_height,
            Variable::BottomHeight => self.bottom_height,
            Variable::MiniIconWidth => self.mini_icon_width,
            Variable::MiniIconHeight => self.mini_icon_height,
            Variable::IconWidth => self.icon_width,
            Variable::IconHeight => self.icon_height,
            Variable::TitleWidth => self.title_width,
            Variable::TitleHeight => self.title_height,
            Variable::FrameXCenter => self.frame_x_center,
            Variable::FrameYCenter => self.frame_y_center,
        })
    }
}

fn unknown_variable(name: &str) -> String {
    format!("Coordinate expression had unknown variable or constant \"{name}\"")
}

impl Expr {
    /// The expression's value in pixels (Marco's `pos_eval`), or the error Marco
    /// reports.
    pub fn eval(&self, env: &ExprEnv) -> Result<i32, String> {
        self.value(env).map(Value::to_int)
    }

    fn value(&self, env: &ExprEnv) -> Result<Value, String> {
        match self {
            Expr::Int(value) => Ok(Value::Int(*value)),
            Expr::Double(value) => Ok(Value::Double(*value)),
            Expr::Variable(variable) => env.get(*variable).map(Value::Int),
            Expr::Binary(a, op, b) => apply(*op, a.value(env)?, b.value(env)?),
            Expr::Invalid(message) => Err(message.clone()),
        }
    }

    /// An x position, offset by the rectangle's (`parse_x_position_unchecked`). Marco
    /// draws an expression it can't evaluate at 0.
    pub fn x(&self, env: &ExprEnv) -> i32 {
        self.eval(env).map_or(0, |value| env.rect.x + value)
    }

    /// A y position, offset by the rectangle's (`parse_y_position_unchecked`).
    pub fn y(&self, env: &ExprEnv) -> i32 {
        self.eval(env).map_or(0, |value| env.rect.y + value)
    }

    /// A size, at least 1 (`parse_size_unchecked`). One Marco can't evaluate is 0.
    pub fn size(&self, env: &ExprEnv) -> i32 {
        self.eval(env).map_or(0, |value| value.max(1))
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
                    None => return Err(unknown_variable(name)),
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

    /// An environment where every variable has its own value.
    fn env() -> ExprEnv {
        ExprEnv {
            rect: Rect {
                x: 1000,
                y: 2000,
                width: 100,
                height: 20,
            },
            object_width: Some(48),
            object_height: Some(24),
            left_width: 3,
            right_width: 4,
            top_height: 30,
            bottom_height: 5,
            mini_icon_width: 16,
            mini_icon_height: 15,
            icon_width: 49,
            icon_height: 47,
            title_width: 120,
            title_height: 19,
            frame_x_center: 190,
            frame_y_center: 50,
        }
    }

    fn eval(text: &str) -> Result<i32, String> {
        parse(text).eval(&env())
    }

    #[test]
    fn each_variable_evaluates_to_its_value_in_the_environment() {
        for (name, value) in [
            ("width", 100),
            ("height", 20),
            ("object_width", 48),
            ("object_height", 24),
            ("left_width", 3),
            ("right_width", 4),
            ("top_height", 30),
            ("bottom_height", 5),
            ("mini_icon_width", 16),
            ("mini_icon_height", 15),
            ("icon_width", 49),
            ("icon_height", 47),
            ("title_width", 120),
            ("title_height", 19),
            ("frame_x_center", 190),
            ("frame_y_center", 50),
        ] {
            assert_eq!(eval(name), Ok(value), "{name}");
        }
    }

    // Marco defines them only while it draws an image or an icon.
    #[test]
    fn object_size_outside_an_image_is_an_unknown_variable() {
        let env = ExprEnv {
            object_width: None,
            object_height: None,
            ..env()
        };
        for name in ["object_width", "object_height"] {
            let error = parse(name).eval(&env).unwrap_err();
            assert!(error.contains(&format!("\"{name}\"")), "{error}");
        }
    }

    #[test]
    fn integer_operators() {
        assert_eq!(eval("width + 7"), Ok(107));
        assert_eq!(eval("width - 7"), Ok(93));
        assert_eq!(eval("width * 3"), Ok(300));
        assert_eq!(eval("width / 3"), Ok(33));
        assert_eq!(eval("width % 7"), Ok(2));
        assert_eq!(eval("width `max` 150"), Ok(150));
        assert_eq!(eval("width `max` 50"), Ok(100));
        assert_eq!(eval("width `min` 150"), Ok(100));
        assert_eq!(eval("width `min` 50"), Ok(50));
    }

    // C's integer division and remainder round towards zero.
    #[test]
    fn integer_division_truncates_towards_zero() {
        assert_eq!(eval("(height - 27) / 2"), Ok(-3));
        assert_eq!(eval("(height - 27) % 2"), Ok(-1));
    }

    #[test]
    fn a_float_makes_the_operation_floating_point() {
        assert_eq!(eval("width * 0.5 + 0.5"), Ok(50));
        assert_eq!(eval("width / 3.0 * 3"), Ok(100));
        assert_eq!(eval("height - 0.5 - 0.5"), Ok(19));
        assert_eq!(eval("width `max` 100.9"), Ok(100));
        assert_eq!(eval("width `min` 99.5"), Ok(99));
    }

    // Marco converts a floating-point result to pixels with a C cast.
    #[test]
    fn a_floating_point_result_truncates_towards_zero() {
        assert_eq!(eval("width * 0.019"), Ok(1));
        assert_eq!(eval("(height - 21) * 0.9"), Ok(0));
        assert_eq!(eval("(height - 22) * 0.9"), Ok(-1));
    }

    #[test]
    fn precedence_and_parentheses_apply_when_evaluating() {
        assert_eq!(eval("width - height * 2"), Ok(60));
        assert_eq!(eval("(width - height) * 2"), Ok(160));
        assert_eq!(eval("width - 10 `max` height + 5"), Ok(90));
        assert_eq!(eval("width - (10 `max` height) + 5"), Ok(85));
    }

    #[test]
    fn evaluation_errors() {
        assert!(eval("width / 0").unwrap_err().contains("division by zero"));
        assert!(eval("width % 0").unwrap_err().contains("division by zero"));
        assert!(
            eval("width / 0.0")
                .unwrap_err()
                .contains("division by zero")
        );
        assert!(eval("width % 2.0").unwrap_err().contains("mod operator"));
        assert!(
            eval("width + Undefined")
                .unwrap_err()
                .contains("\"Undefined\"")
        );
    }

    #[test]
    fn a_constant_ignores_the_environment() {
        assert_eq!(eval("ButtonWidth / 2"), Ok(9));
    }

    #[test]
    fn a_position_is_offset_by_the_rectangle() {
        assert_eq!(parse("width - 1").x(&env()), 1099);
        assert_eq!(parse("height / 2").y(&env()), 2010);
        assert_eq!(parse("3").x(&env()), 1003);
    }

    #[test]
    fn a_size_is_at_least_one() {
        assert_eq!(parse("width - 1").size(&env()), 99);
        assert_eq!(parse("width - 100").size(&env()), 1);
        assert_eq!(parse("0 - 5").size(&env()), 1);
    }

    // Marco draws with 0, not the rectangle's origin or 1, when it can't evaluate.
    #[test]
    fn a_failed_expression_is_zero_for_positions_and_sizes() {
        assert_eq!(parse("width / 0").x(&env()), 0);
        assert_eq!(parse("width / 0").y(&env()), 0);
        assert_eq!(parse("width / 0").size(&env()), 0);
        assert_eq!(parse("Undefined").size(&env()), 0);
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
