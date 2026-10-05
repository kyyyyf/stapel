//! UTC timestamps in RFC 3339 without a date library.

use std::time::{SystemTime, UNIX_EPOCH};

/// The current time as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339(secs)
}

pub fn rfc3339(unix_secs: u64) -> String {
    let days = (unix_secs / 86_400) as i64;
    let rem = unix_secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (H. Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (inverse of `civil_from_days`).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Milliseconds since 1970 of `YYYY-MM-DDTHH:MM:SS[.fff…]Z`, or `None`.
fn parse_millis(s: &str, fraction_allowed: bool) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = s.get(r)?;
        part.bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&mo)
        || d < 1
        || d > month_days[mo as usize - 1]
        || h > 23
        || mi > 59
        || se > 59
    {
        return None;
    }
    let rest = &s[19..];
    let millis = if rest == "Z" {
        0
    } else if fraction_allowed && rest.starts_with('.') && rest.ends_with('Z') {
        let digits = &rest[1..rest.len() - 1];
        if digits.is_empty() || digits.len() > 9 || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let padded = format!("{digits:0<3}");
        padded[..3].parse::<u64>().ok()?
    } else {
        return None;
    };
    let days = days_from_civil(i64::from(y), mo, d);
    let secs = days * 86_400 + i64::from(h) * 3600 + i64::from(mi) * 60 + i64::from(se);
    u64::try_from(secs).ok().map(|s| s * 1000 + millis)
}

/// Seconds since 1970 of exactly `YYYY-MM-DDTHH:MM:SSZ`, the form `stapel` writes and accepts.
pub fn parse_utc(s: &str) -> Option<u64> {
    (s.len() == 20)
        .then(|| parse_millis(s, false))
        .flatten()
        .map(|ms| ms / 1000)
}

/// Milliseconds since 1970 of a Claude Code transcript timestamp, `…:SS.mmmZ` or `…:SSZ`.
pub fn parse_transcript_time(s: &str) -> Option<u64> {
    parse_millis(s, true)
}

/// Milliseconds since 1970 of a time given on the command line: `YYYY-MM-DDTHH:MM:SSZ`, optionally
/// with a fraction of a second (`…:SS.mmmZ`), the form of a record's `from` and `to`.
pub fn parse_cli_time(s: &str) -> Option<u64> {
    parse_millis(s, true)
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` for milliseconds since 1970.
pub fn rfc3339_millis(ms: u64) -> String {
    let base = rfc3339(ms / 1000);
    format!("{}.{:03}Z", &base[..19], ms % 1000)
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_dates() {
        assert_eq!(super::rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(super::rfc3339(1_790_983_200), "2026-10-02T23:20:00Z");
        assert_eq!(super::rfc3339(951_782_400), "2000-02-29T00:00:00Z");
    }
}
