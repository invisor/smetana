//! The pause between two batches of an autopilot run: how long, drawn from the
//! interval the person chose.
//!
//! Pure on purpose. The loop in `service.rs` owns the waiting, the state and the
//! journal line; this file owns the one decision that is not mechanical, which is
//! how many minutes, and takes its randomness as an argument so a test can hand
//! it anything.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

/// The longest the wait sleeps in one piece. A tokio sleep runs on a monotonic
/// clock that does not advance while a laptop is asleep, so one long sleep would
/// outlast the `until` the bar is showing; slicing against the wall clock lets
/// the loop notice on waking that the moment has passed.
const SLICE: Duration = Duration::from_secs(60);

/// Wait until the wall clock `now` reads `until`, or until a stop arrives.
/// `true` means the pause ran out and the loop goes on; `false` means stop.
///
/// The clock is an argument so a test under paused tokio time can supply one that
/// moves with the virtual clock — `Utc::now` does not, and would never reach
/// `until`. The loop passes `Utc::now`.
pub async fn wait(
    until: DateTime<Utc>,
    now: impl Fn() -> DateTime<Utc>,
    stop: &mut mpsc::Receiver<()>,
) -> bool {
    loop {
        let left = (until - now()).to_std().unwrap_or(Duration::ZERO);
        if left.is_zero() {
            return true;
        }
        tokio::select! {
            _ = tokio::time::sleep(left.min(SLICE)) => {}
            _ = stop.recv() => return false,
        }
    }
}

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

    // Wall time that moves with tokio's virtual clock, so `wait` can be driven
    // through twenty minutes without any of them passing.
    fn virtual_clock() -> impl Fn() -> DateTime<Utc> {
        let began = tokio::time::Instant::now();
        let at = Utc::now();
        move || at + chrono::Duration::from_std(began.elapsed()).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn the_wait_ends_when_the_interval_has_passed() {
        let (_keep, mut stop) = mpsc::channel::<()>(1);
        let now = virtual_clock();
        let until = now() + chrono::Duration::minutes(20);
        let began = tokio::time::Instant::now();
        assert!(wait(until, &now, &mut stop).await, "a pause that ran out lets the run go on");
        assert!(began.elapsed() >= Duration::from_secs(20 * 60), "{:?}", began.elapsed());
        assert!(began.elapsed() < Duration::from_secs(20 * 60 + 61));
    }

    #[tokio::test(start_paused = true)]
    async fn a_stop_in_the_middle_ends_it_at_once() {
        let (tx, mut stop) = mpsc::channel::<()>(1);
        let now = virtual_clock();
        let until = now() + chrono::Duration::minutes(20);
        let began = tokio::time::Instant::now();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5 * 60)).await;
            let _ = tx.send(()).await;
        });
        assert!(!wait(until, &now, &mut stop).await, "stop must end the pause");
        assert!(began.elapsed() < Duration::from_secs(5 * 60 + 1), "{:?}", began.elapsed());
    }

    #[tokio::test(start_paused = true)]
    async fn a_moment_already_past_returns_without_sleeping() {
        let (_keep, mut stop) = mpsc::channel::<()>(1);
        let now = virtual_clock();
        let began = tokio::time::Instant::now();
        assert!(wait(now() - chrono::Duration::minutes(1), &now, &mut stop).await);
        assert_eq!(began.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn a_clock_that_jumped_ahead_ends_the_wait_early() {
        // A laptop that slept: the wall clock is far ahead of the monotonic one,
        // and the next slice must see it rather than sleep the whole interval.
        let (_keep, mut stop) = mpsc::channel::<()>(1);
        let began = tokio::time::Instant::now();
        let at = Utc::now();
        let now = move || at + chrono::Duration::from_std(began.elapsed()).unwrap() * 60;
        let until = at + chrono::Duration::minutes(20);
        assert!(wait(until, &now, &mut stop).await);
        assert!(began.elapsed() <= Duration::from_secs(60), "{:?}", began.elapsed());
    }
}
