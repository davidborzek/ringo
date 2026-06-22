//! Duration parsing shared by `await_until`/`default_timeout` (language-neutral).

use std::time::Duration;

/// "10s" / "500ms" / "2m" → Duration. `Err` is a human message.
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let split = s.find(|c: char| c.is_alphabetic());
    let (num, unit) = match split {
        Some(i) => (&s[..i], &s[i..]),
        None => return Err(format!("invalid duration `{s}` (use e.g. 10s, 500ms)")),
    };
    let n: u64 = num
        .trim()
        .parse()
        .map_err(|_| format!("invalid duration number in `{s}`"))?;
    Ok(match unit {
        "ms" => Duration::from_millis(n),
        "s" => Duration::from_secs(n),
        "m" => Duration::from_secs(
            n.checked_mul(60)
                .ok_or_else(|| format!("duration `{s}` is too large"))?,
        ),
        other => return Err(format!("unknown duration unit `{other}` (use ms/s/m)")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_units() {
        assert_eq!(parse_duration("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse_duration("10s").unwrap(), Duration::from_secs(10));
        assert_eq!(parse_duration("2m").unwrap(), Duration::from_secs(120));
        assert!(parse_duration("nope").is_err());
        assert!(parse_duration("5x").is_err());
        // overflow in the `*60` for minutes is reported, not a panic
        assert!(parse_duration("100000000000000000000m").is_err());
        assert!(parse_duration(&format!("{}m", u64::MAX)).is_err());
    }
}
