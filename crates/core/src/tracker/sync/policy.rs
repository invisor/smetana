//! Fixed accumulation windows and retry deadlines. All times are monotonic.
use tokio::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub struct Policy {
    generation: u64,
    pending: Option<(u64, Instant)>,
    in_flight: Option<(u64, Instant)>,
    retry_not_before: Option<Instant>,
}

impl Policy {
    pub fn changed(&mut self, now: Instant) {
        self.generation += 1;
        if self.pending.is_none() {
            self.pending = Some((self.generation, now + WINDOW));
        }
    }

    pub fn due(&self, now: Instant) -> bool {
        self.pending.is_some_and(|(_, deadline)| deadline <= now)
            && self.retry_not_before.is_none_or(|retry| retry <= now)
    }

    pub fn begin_push(&mut self) -> Option<u64> {
        let (_, deadline) = self.pending.take()?;
        let through = self.generation;
        self.in_flight = Some((through, deadline));
        Some(through)
    }

    pub fn finish_push(&mut self, through: u64, now: Instant, success: bool) {
        let former = self.in_flight.take();
        if success {
            self.retry_not_before = None;
        } else {
            if let Some((_, deadline)) = former {
                self.pending = Some((
                    through,
                    deadline.min(self.pending.map(|(_, d)| d).unwrap_or(deadline)),
                ));
            }
            self.retry_not_before = Some(now + WINDOW);
        }
    }

    #[cfg(test)]
    fn deadline(&self) -> Option<Instant> {
        self.pending.map(|(_, deadline)| deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn writes_at_zero_twenty_and_fifty_nine_share_one_deadline() {
        let mut policy = Policy::default();
        let start = Instant::now();
        policy.changed(start);
        tokio::time::advance(Duration::from_secs(20)).await;
        policy.changed(Instant::now());
        tokio::time::advance(Duration::from_secs(39)).await;
        policy.changed(Instant::now());
        assert_eq!(policy.deadline(), Some(start + WINDOW));
        assert!(!policy.due(Instant::now()));
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(policy.due(Instant::now()));
    }

    #[tokio::test(start_paused = true)]
    async fn second_generation_and_failed_retry_keep_their_own_deadlines() {
        let mut policy = Policy::default();
        policy.changed(Instant::now());
        let first = policy.begin_push().unwrap();
        tokio::time::advance(Duration::from_secs(10)).await;
        policy.changed(Instant::now());
        let second_deadline = policy.deadline();
        policy.finish_push(first, Instant::now(), true);
        assert_eq!(policy.deadline(), second_deadline);
        let second = policy.begin_push().unwrap();
        policy.finish_push(second, Instant::now(), false);
        tokio::time::advance(Duration::from_secs(59)).await;
        policy.changed(Instant::now());
        assert!(!policy.due(Instant::now()));
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(policy.due(Instant::now()));
    }
}
