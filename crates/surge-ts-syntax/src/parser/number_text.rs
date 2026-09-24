//! ECMAScript `Number::toString`, which is how tsc spells a numeric literal
//! (the scanner stores `jsnum.FromString(text).String()`): a property named
//! `1e1000` is `Infinity`, and `9671406556917009000000000` is
//! `9.671406556917009e+24`, where Rust's `Display` writes the digits out in
//! full and non-finite values as `inf`/`NaN`.

pub fn js_number_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if value < 0.0 {
        return format!("-{}", js_number_to_string(-value));
    }
    // `{:e}` writes the shortest digits that round-trip, which is the digit
    // string `Number::toString` lays out.
    let scientific = format!("{value:e}");
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return scientific;
    };
    let Ok(exponent) = exponent.parse::<i32>() else {
        return scientific;
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let digit_count = digits.len() as i32;
    let point = exponent + 1;
    if digit_count <= point && point <= 21 {
        format!("{digits}{}", "0".repeat((point - digit_count) as usize))
    } else if 0 < point && point <= 21 {
        format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
    } else if -6 < point && point <= 0 {
        format!("0.{}{digits}", "0".repeat((-point) as usize))
    } else {
        let sign = if exponent < 0 { '-' } else { '+' };
        let magnitude = exponent.abs();
        if digit_count == 1 {
            format!("{digits}e{sign}{magnitude}")
        } else {
            format!("{}.{}e{sign}{magnitude}", &digits[..1], &digits[1..])
        }
    }
}
