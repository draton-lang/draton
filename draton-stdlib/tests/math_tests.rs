use draton_stdlib::math;

#[test]
fn floating_math_helpers_match_expected_values() {
    assert!((math::sqrt(81.0) - 9.0).abs() < 1e-12);
    assert!((math::pow(2.0, 5.0) - 32.0).abs() < 1e-12);
    assert!((math::abs(-3.5) - 3.5).abs() < 1e-12);
    assert_eq!(math::floor(3.9), 3.0);
    assert_eq!(math::ceil(3.1), 4.0);
    assert_eq!(math::round(3.6), 4.0);
    assert!((math::sin(0.0) - 0.0).abs() < 1e-12);
    assert!((math::cos(0.0) - 1.0).abs() < 1e-12);
    assert_eq!(math::min(1.0, 2.0), 1.0);
    assert_eq!(math::max(1.0, 2.0), 2.0);
    assert_eq!(math::clamp(10.0, 0.0, 5.0), 5.0);
    assert!(math::pi() > 3.14);
    assert!(math::e() > 2.71);
}

#[test]
fn checked_integer_helpers_report_overflow_and_div_zero() {
    assert_eq!(math::checked_add(1, 2), Some(3));
    assert_eq!(math::checked_sub(9, 4), Some(5));
    assert_eq!(math::checked_mul(3, 7), Some(21));
    assert_eq!(math::checked_div(20, 5), Some(4));
    assert_eq!(math::checked_add(i64::MAX, 1), None);
    assert_eq!(math::checked_mul(i64::MAX, 2), None);
    assert_eq!(math::checked_div(1, 0), None);
}
