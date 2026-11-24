// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the Python-compatible float rendering.

use pretty_assertions::assert_eq;

use super::python_float_repr;

#[test]
fn test_format_python_float_repr_whole_seconds_appends_point_zero() {
    assert_eq!(python_float_repr(6000.0), "6000.0");
}

#[test]
fn test_format_python_float_repr_zero_is_point_zero() {
    assert_eq!(python_float_repr(0.0), "0.0");
}

#[test]
fn test_format_python_float_repr_negative_zero_keeps_the_sign() {
    assert_eq!(python_float_repr(-0.0), "-0.0");
}

#[test]
fn test_format_python_float_repr_fraction_stays_plain() {
    assert_eq!(python_float_repr(1.5), "1.5");
}

#[test]
fn test_format_python_float_repr_half_stays_plain() {
    assert_eq!(python_float_repr(0.5), "0.5");
}

#[test]
fn test_format_python_float_repr_negative_fraction_keeps_the_sign() {
    assert_eq!(python_float_repr(-2.5), "-2.5");
}

#[test]
fn test_format_python_float_repr_exact_integer_appends_point_zero() {
    assert_eq!(python_float_repr(1234.0), "1234.0");
}

#[test]
fn test_format_python_float_repr_hundredth_stays_fixed() {
    assert_eq!(python_float_repr(0.01), "0.01");
}

#[test]
fn test_format_python_float_repr_tenth_millisecond_stays_fixed() {
    assert_eq!(python_float_repr(0.0001), "0.0001");
}

#[test]
fn test_format_python_float_repr_hundred_thousandth_is_scientific() {
    assert_eq!(python_float_repr(0.00001), "1e-05");
}

#[test]
fn test_format_python_float_repr_microsecond_is_scientific() {
    assert_eq!(python_float_repr(0.000_001), "1e-06");
}

#[test]
fn test_format_python_float_repr_fifteen_digit_power_stays_fixed() {
    assert_eq!(python_float_repr(1e15), "1000000000000000.0");
}

#[test]
fn test_format_python_float_repr_sixteen_digit_power_is_scientific() {
    assert_eq!(python_float_repr(1e16), "1e+16");
}

#[test]
fn test_format_python_float_repr_two_significant_digits_is_scientific() {
    assert_eq!(python_float_repr(1.5e20), "1.5e+20");
}

#[test]
fn test_format_python_float_repr_three_significant_digits_is_scientific() {
    assert_eq!(python_float_repr(1.25e20), "1.25e+20");
}
