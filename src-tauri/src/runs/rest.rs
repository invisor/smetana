//! The pause between two batches of an autopilot run: how long, drawn from the
//! interval the person chose.
//!
//! Pure on purpose. The loop in `service.rs` owns the waiting, the state and the
//! journal line; this file owns the one decision that is not mechanical, which is
//! how many minutes, and takes its randomness as an argument so a test can hand
//! it anything.

use std::time::{SystemTime, UNIX_EPOCH};

/// One whole number of minutes out of `[min, max]`, both ends included.
///
/// `rng` yields a uniformly distributed `u64` per call. A reversed pair is read
/// the right way round rather than panicking: `RunSettings::validate` refuses
/// one at the door, and a loop task that died on a bad pair would be the worse
/// answer to a mistake it should never see.
pub fn pick(min: u16, max: u16, mut rng: impl FnMut() -> u64) -> u16 {
    let (low, high) = if min <= max { (min, max) } else { (max, min) };
    if low == high {
        return low;
    }
    let span = u64::from(high - low) + 1;
    // The modulo is biased by at most span / 2^64, which for a span of a
    // few hundred is not a number; the interval is a courtesy to the machine,
    // not a lottery.
    low + (rng() % span) as u16
}

/// A source for `pick` that needs no crate: splitmix64 over a counter seeded
/// from the clock. Not for anything that must resist a guess — it spaces
/// batches out, and the only property asked of it is that two runs do not draw
/// in step.
pub fn clock_rng() -> impl FnMut() -> u64 {
    let mut state = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draw_stays_inside_the_interval_with_both_ends_reachable() {
        let mut rng = clock_rng();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..2000 {
            let minutes = pick(10, 13, &mut rng);
            assert!((10..=13).contains(&minutes), "{minutes} is outside 10..=13");
            seen.insert(minutes);
        }
        assert_eq!(seen.len(), 4, "every minute of a small interval turns up: {seen:?}");
    }

    #[test]
    fn the_ends_are_what_the_extremes_of_the_source_give() {
        assert_eq!(pick(10, 30, || 0), 10);
        assert_eq!(pick(10, 30, || 20), 30, "span 21: 20 is the last residue");
        assert_eq!(pick(10, 30, || u64::MAX), 10 + (u64::MAX % 21) as u16);
    }

    #[test]
    fn an_interval_of_one_number_is_that_number() {
        assert_eq!(pick(15, 15, || 12345), 15);
        assert_eq!(pick(0, 0, || 7), 0);
    }

    #[test]
    fn the_whole_range_fits() {
        assert_eq!(pick(0, 720, || 720), 720);
        assert!(pick(0, u16::MAX, || u64::MAX) <= u16::MAX);
    }

    #[test]
    fn a_reversed_pair_is_read_the_right_way_round() {
        assert!((3..=9).contains(&pick(9, 3, clock_rng())));
    }
}
