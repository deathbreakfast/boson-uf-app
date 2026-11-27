//! Short labels shared by the Boson tables: ids, counts, durations and waits.

/// Last four characters of an id after "…"; ids of four characters or fewer show whole.
pub fn short_id(id: &str) -> String {
    let count = id.chars().count();
    if count <= 4 {
        return id.to_string();
    }
    let tail: String = id.chars().skip(count - 4).collect();
    format!("…{tail}")
}

/// A count with thousands separators: `4812` reads "4,812".
pub fn format_count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A count and its noun, singular for one: "1 run", "4,812 runs".
pub fn count_noun(n: u64, singular: &str, plural: &str) -> String {
    format!(
        "{} {}",
        format_count(n),
        if n == 1 { singular } else { plural }
    )
}

/// A run duration in seconds with one decimal: "30.0 s", or "-" when unknown.
pub fn format_seconds(ms: Option<i64>) -> String {
    ms.map_or_else(
        || "-".to_string(),
        |ms| {
            #[allow(clippy::cast_precision_loss)]
            let secs = ms.max(0) as f64 / 1000.0;
            format!("{secs:.1} s")
        },
    )
}

/// How long something has waited, at a glance: "45 s", "6 min", "3 h", "2 d".
pub fn format_waited(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    match secs {
        0..=59 => format!("{secs} s"),
        60..=3_599 => format!("{} min", secs / 60),
        3_600..=172_799 => format!("{} h", secs / 3_600),
        _ => format!("{} d", secs / 86_400),
    }
}

/// "3 of 3" when the most attempts is known, else just the attempt.
pub fn format_attempts(attempt: i32, max_attempts: Option<u32>) -> String {
    max_attempts.map_or_else(|| attempt.to_string(), |max| format!("{attempt} of {max}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_labels() {
        assert_eq!(short_id("00000000-0000-4000-8000-000000000101"), "…0101");
        assert_eq!(short_id("abc"), "abc");
        assert_eq!(format_count(4812), "4,812");
        assert_eq!(format_count(1_000_000), "1,000,000");
        assert_eq!(format_count(999), "999");
        assert_eq!(count_noun(1, "run", "runs"), "1 run");
        assert_eq!(format_seconds(Some(30_000)), "30.0 s");
        assert_eq!(format_seconds(Some(1_849)), "1.8 s");
        assert_eq!(format_seconds(None), "-");
        assert_eq!(format_waited(6 * 60_000 + 5_000), "6 min");
        assert_eq!(format_waited(-5), "0 s");
        assert_eq!(format_attempts(3, Some(3)), "3 of 3");
        assert_eq!(format_attempts(2, None), "2");
    }
}
