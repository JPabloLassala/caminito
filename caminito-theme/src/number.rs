//! C's number parsing, as Marco's parser relies on it.

/// `g_ascii_strtod`, which is C's `strtod` in the C locale: the longest prefix of `text`
/// that is a decimal or hexadecimal floating-point number, an infinity or a NaN, after
/// leading whitespace, and how many bytes it used. `None` if there is none.
pub(crate) fn strtod_prefix(text: &str) -> Option<(f64, usize)> {
    let (start, negative) = skip_space_and_sign(text);
    let rest = &text[start..];
    let lower = rest.to_ascii_lowercase();
    let (magnitude, len) = if lower.starts_with("inf") {
        let len = if lower.starts_with("infinity") { 8 } else { 3 };
        (f64::INFINITY, len)
    } else if lower.starts_with("nan") {
        // C also takes nan(chars), which Marco's themes never write.
        (f64::NAN, 3)
    } else if lower.starts_with("0x") && hex_mantissa_len(&rest.as_bytes()[2..]) > 0 {
        hex_float(rest)
    } else {
        decimal_float(rest)?
    };
    let value = if negative { -magnitude } else { magnitude };
    Some((value, start + len))
}

/// `strtol` in base 10: the value of the longest prefix of `text` that is an integer,
/// after leading whitespace, saturating as strtol does, and how many bytes it used.
pub(crate) fn strtol_prefix(text: &str) -> Option<(i64, usize)> {
    let (start, negative) = skip_space_and_sign(text);
    let digits = count_digits(&text.as_bytes()[start..], 10);
    if digits == 0 {
        return None;
    }
    let end = start + digits;
    let value = text[start..end]
        .parse::<i64>()
        .map(|value| if negative { -value } else { value })
        .unwrap_or(if negative { i64::MIN } else { i64::MAX });
    Some((value, end))
}

/// Where the number starts after leading whitespace and a sign, and whether the sign was
/// a minus.
fn skip_space_and_sign(text: &str) -> (usize, bool) {
    let bytes = text.as_bytes();
    let mut i = bytes.iter().take_while(|c| c.is_ascii_whitespace()).count();
    let negative = bytes.get(i) == Some(&b'-');
    if matches!(bytes.get(i), Some(b'+' | b'-')) {
        i += 1;
    }
    (i, negative)
}

/// A decimal float without its sign: digits, a point and digits, and an exponent.
fn decimal_float(text: &str) -> Option<(f64, usize)> {
    let bytes = text.as_bytes();
    let int_digits = count_digits(bytes, 10);
    let mut i = int_digits;
    let mut frac_digits = 0;
    if bytes.get(i) == Some(&b'.') {
        frac_digits = count_digits(&bytes[i + 1..], 10);
        i += 1 + frac_digits;
    }
    if int_digits == 0 && frac_digits == 0 {
        return None;
    }
    i += exponent_len(&bytes[i..], b'e');
    text[..i].parse().ok().map(|value| (value, i))
}

/// A hexadecimal float without its sign, `0x` and hex digits with an optional point and
/// binary exponent.
fn hex_float(text: &str) -> (f64, usize) {
    let bytes = &text.as_bytes()[2..];
    let mut value = 0.0;
    let mut scale = 0i32;
    let mut seen_point = false;
    let mut i = 0;
    while let Some(&c) = bytes.get(i) {
        if c == b'.' && !seen_point {
            seen_point = true;
        } else if let Some(digit) = (c as char).to_digit(16) {
            value = value * 16.0 + f64::from(digit);
            if seen_point {
                scale -= 4;
            }
        } else {
            break;
        }
        i += 1;
    }
    let exponent_len = exponent_len(&bytes[i..], b'p');
    if exponent_len > 0 {
        scale += text[2 + i + 1..2 + i + exponent_len]
            .parse::<i32>()
            .unwrap_or(0);
    }
    (value * 2f64.powi(scale), 2 + i + exponent_len)
}

fn hex_mantissa_len(bytes: &[u8]) -> usize {
    let int_digits = count_digits(bytes, 16);
    match bytes.get(int_digits) {
        Some(b'.') => int_digits + count_digits(&bytes[int_digits + 1..], 16),
        _ => int_digits,
    }
}

/// The length of an exponent (`e` or `p`, a sign and digits) at the start of `bytes`, or
/// 0 if there isn't a whole one.
fn exponent_len(bytes: &[u8], marker: u8) -> usize {
    if !bytes
        .first()
        .is_some_and(|c| c.to_ascii_lowercase() == marker)
    {
        return 0;
    }
    let sign = usize::from(matches!(bytes.get(1), Some(b'+' | b'-')));
    match count_digits(&bytes[1 + sign..], 10) {
        0 => 0,
        digits => 1 + sign + digits,
    }
}

fn count_digits(bytes: &[u8], radix: u32) -> usize {
    bytes
        .iter()
        .take_while(|&&c| (c as char).is_digit(radix))
        .count()
}
