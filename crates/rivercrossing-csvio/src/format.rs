// SPDX-License-Identifier: GPL-3.0-only

//! The Python-compatible float rendering that the CSV exporters share.

/// The exponent range that Python's `repr` renders without scientific notation.
///
/// The range is inclusive at the bottom and exclusive at the top, so the
/// decimal exponent `-4` stays fixed and `16` turns scientific.
const FIXED_EXPONENT_RANGE: std::ops::Range<i32> = -4..16;

/// Render a ride duration the way Python's `repr` renders a float.
///
/// The shortest round-trip digits render in fixed notation inside Python's
/// fixed range and in scientific notation outside it, with a two-digit signed
/// exponent. A whole value keeps its trailing `.0`, and negative zero keeps its
/// sign.
#[must_use]
pub(crate) fn python_float_repr(value: f64) -> String {
    if value == 0.0 {
        return zero_repr(value);
    }
    let text = format!("{value}");
    let (sign, magnitude) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text.as_str()),
    };
    let (integer, fraction) = magnitude.split_once('.').unwrap_or((magnitude, ""));
    let decimal_point = if integer == "0" {
        -count_leading_zeros(fraction)
    } else {
        count_chars(integer)
    };
    let raw_digits = format!("{integer}{fraction}");
    let digits = raw_digits.trim_start_matches('0').trim_end_matches('0');
    let exponent = decimal_point - 1;
    if FIXED_EXPONENT_RANGE.contains(&exponent) {
        format!("{sign}{}", render_fixed(digits, decimal_point))
    } else {
        format!("{sign}{}", render_scientific(digits, exponent))
    }
}

/// Render a zero with the sign that its bit pattern carries.
fn zero_repr(value: f64) -> String {
    if value.is_sign_negative() {
        return "-0.0".to_owned();
    }
    "0.0".to_owned()
}

/// Count the characters of *text* as a decimal exponent.
fn count_chars(text: &str) -> i32 {
    text.chars().fold(0_i32, |count, _| count + 1)
}

/// Count the leading zero characters of *text*.
fn count_leading_zeros(text: &str) -> i32 {
    count_chars(text) - count_chars(text.trim_start_matches('0'))
}

/// Render *digits* in fixed notation with the decimal point after
/// *`decimal_point`* digits.
fn render_fixed(digits: &str, decimal_point: i32) -> String {
    if decimal_point <= 0 {
        let mut rendered = String::from("0.");
        for _ in 0..-decimal_point {
            rendered.push('0');
        }
        rendered.push_str(digits);
        return rendered;
    }
    let mut rendered = String::new();
    let mut remaining = decimal_point;
    let mut has_fraction = false;
    for digit in digits.chars() {
        if remaining == 0 {
            rendered.push('.');
            has_fraction = true;
        }
        rendered.push(digit);
        remaining -= 1;
    }
    if has_fraction {
        return rendered;
    }
    for _ in 0..remaining {
        rendered.push('0');
    }
    rendered.push_str(".0");
    rendered
}

/// Render *digits* in scientific notation with *exponent*.
fn render_scientific(digits: &str, exponent: i32) -> String {
    let mut mantissa = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index == 1 {
            mantissa.push('.');
        }
        mantissa.push(digit);
    }
    format!("{mantissa}e{exponent:+03}")
}

#[cfg(test)]
mod tests;
