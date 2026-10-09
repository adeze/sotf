//! Native numeric metadata must not fabricate readback or slider ranges.

#[path = "../components/plugins/external_numeric.rs"]
mod external_numeric;

use external_numeric::NumericControl;
use sotf_plugins::ParameterValue::{self, Bool, Float, Int};

#[test]
fn unbounded_and_one_sided_parameters_retain_exact_editing_without_a_slider() {
    let unbounded = NumericControl::from_native(&Float(2.5), None, None).unwrap();
    assert_eq!(unbounded.current, 2.5);
    assert!(unbounded.slider_range.is_none());
    assert_eq!(unbounded.encode(3.125), Some(Float(3.125)));
    let lower = NumericControl::from_native(&Float(2.5), Some(&Float(0.0)), None).unwrap();
    assert!(lower.slider_range.is_none());
    assert_eq!(lower.encode(-1.0), None);
    let upper = NumericControl::from_native(&Int(12), None, Some(&Int(20))).unwrap();
    assert!(upper.integer);
    assert!(upper.slider_range.is_none());
    assert_eq!(upper.encode(20.0), Some(Int(20)));
    assert_eq!(upper.encode(21.0), None);
}

#[test]
fn bounded_controls_preserve_native_value_type_and_exact_endpoints() {
    let control =
        NumericControl::from_native(&Float(1.25), Some(&Float(0.0)), Some(&Float(4.0))).unwrap();
    assert_eq!(control.current, 1.25);
    assert_eq!(control.slider_range, Some((0.0, 4.0)));
    assert_eq!(control.encode(1.0), Some(Float(1.0)));
    assert_eq!(control.encode(4.0), Some(Float(4.0)));
    assert_eq!(control.encode(4.001), None);
}

#[test]
fn integer_bounds_do_not_silently_saturate_or_lose_precision() {
    let control =
        NumericControl::from_native(&Int(2), Some(&Float(1.2)), Some(&Float(4.8))).unwrap();
    assert_eq!((control.min, control.max), (2.0, 4.0));
    assert_eq!(control.encode(2.7), Some(Int(3)));
    assert_eq!(control.encode(4.2), None);
    let large =
        NumericControl::from_native(&Int(i32::MAX), Some(&Int(0)), Some(&Int(i32::MAX))).unwrap();
    assert!(large.slider_range.is_none());
    assert_eq!(large.current, f64::from(i32::MAX));
    assert_eq!(large.encode(f64::from(i32::MAX)), Some(Int(i32::MAX)));
    assert_eq!(large.encode(f64::from(i32::MAX) + 1.0), None);
}

#[test]
fn invalid_native_metadata_or_readback_never_becomes_an_editable_default() {
    let cases = [
        (Float(f32::NAN), None, None),
        (Float(f32::INFINITY), None, None),
        (Float(2.0), Some(Float(f32::NAN)), None),
        (Float(2.0), None, Some(Float(f32::INFINITY))),
        (Float(2.0), Some(Bool(false)), None),
        (Float(2.0), Some(Float(3.0)), Some(Float(1.0))),
        (Float(2.0), Some(Float(0.0)), Some(Float(1.0))),
        (Int(2), Some(Float(2.1)), Some(Float(2.9))),
    ];
    for (value, min, max) in cases {
        assert!(NumericControl::from_native(&value, min.as_ref(), max.as_ref()).is_err());
    }
    let control = NumericControl::from_native(&Float(2.0), None, None).unwrap();
    for value in [f64::NAN, f64::INFINITY, -f64::INFINITY, f64::MAX] {
        assert_eq!(control.encode(value), None);
    }
}

#[test]
fn fixed_native_ranges_are_truthful_and_have_no_slider() {
    let fixed = NumericControl::from_native(&Int(3), Some(&Int(3)), Some(&Int(3))).unwrap();
    assert_eq!(fixed.min, fixed.max);
    assert!(fixed.slider_range.is_none());
    assert_eq!(fixed.encode(3.0), Some(ParameterValue::Int(3)));
    assert_eq!(fixed.encode(2.0), None);
}
