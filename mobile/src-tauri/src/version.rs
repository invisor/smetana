//! Comparing app versions: the one rule that decides whether a desktop's
//! `min_mobile_version` shuts a phone out.

use std::cmp::Ordering;

/// `major.minor.patch` as numbers. A missing part is 0, a suffix after `-` or `+`
/// is ignored, and a part that is not a number is read as 0: a desktop that sends
/// something odd must not lock a phone out for it.
fn parts(version: &str) -> [u64; 3] {
    let core = version.trim().split(['-', '+']).next().unwrap_or("");
    let mut out = [0; 3];
    for (slot, part) in out.iter_mut().zip(core.split('.')) {
        *slot = part.trim().parse().unwrap_or(0);
    }
    out
}

pub fn compare(a: &str, b: &str) -> Ordering {
    parts(a).cmp(&parts(b))
}

/// Whether `have` is older than `wanted`.
pub fn older_than(have: &str, wanted: &str) -> bool {
    compare(have, wanted) == Ordering::Less
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_and_not_by_text() {
        assert!(older_than("0.9.0", "0.10.0"));
        assert!(older_than("1.2", "1.2.1"));
        assert!(!older_than("1.2.0", "1.2"));
        assert!(!older_than("2.0.0", "1.99.99"));
        assert!(older_than("0.1.0", "0.1.1-beta"));
    }

    #[test]
    fn odd_input_never_locks_a_phone_out() {
        assert!(!older_than("0.1.0", ""));
        assert!(!older_than("0.1.0", "garbage"));
        assert!(!older_than("0.1.0", "0.1.0+build5"));
    }
}
