//! Pure selection of the next run-lead harness after an allowance decision.
//!
//! This module deliberately has no process, settings-file, or probe I/O. The
//! service supplies a normalized snapshot only at a boundary where no attempt
//! is writing, which makes its ordering safe to test and prevents a recovered
//! primary from pre-empting useful reserve work.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};

use crate::settings::model::RunFailoverSettings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    Ready,
    /// An allowance was read and is unusable. `None` is a real unknown reset,
    /// not a failure to read the percentage.
    Limited { pct: u8, reset_at: Option<DateTime<Utc>>, resets: Option<String> },
    /// Fail open: an unreadable allowance may still start a trial attempt.
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    NewBatch,
    ForcedHandoff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    Stay { agent: String },
    WaitOne { agent: String, until: Option<DateTime<Utc>>, pct: u8, resets: Option<String> },
    Start { agent: String },
    WaitAny { wake_at: DateTime<Utc> },
}

/// Select an installed agent from a complete point-in-time probe map.
pub fn select(
    policy: &RunFailoverSettings,
    primary: &str,
    current: Option<&str>,
    installed: &[String],
    probes: &BTreeMap<String, Probe>,
    boundary: Boundary,
    now: DateTime<Utc>,
) -> Next {
    let current_probe = current.and_then(|id| probes.get(id));
    if !policy.enabled {
        return match current_probe {
            Some(Probe::Limited { pct, reset_at, resets }) => Next::WaitOne {
                agent: current.unwrap_or(primary).to_owned(),
                until: *reset_at,
                pct: *pct,
                resets: resets.clone(),
            },
            _ => Next::Stay { agent: primary.to_owned() },
        };
    }

    if let (Some(agent), Some(Probe::Limited { pct, reset_at, resets })) = (current, current_probe) {
        if reset_at.is_some_and(|at| at >= now && at - now <= Duration::minutes(policy.wait_minutes.into())) {
            return Next::WaitOne {
                agent: agent.to_owned(),
                until: *reset_at,
                pct: *pct,
                resets: resets.clone(),
            };
        }
    }

    // With return disabled, a healthy reserve retains the next logical batch;
    // this is the non-preemption promise expressed at a batch boundary.
    if matches!(boundary, Boundary::NewBatch) && !policy.return_to_primary {
        if let (Some(agent), Some(Probe::Ready | Probe::Unreadable)) = (current, current_probe) {
            return Next::Stay { agent: agent.to_owned() };
        }
    }

    let mut reserves: Vec<&str> = policy
        .priority
        .iter()
        .map(String::as_str)
        .filter(|id| *id != primary && installed.iter().any(|available| available == id))
        .collect();
    let primary_installed = installed.iter().any(|available| available == primary);
    let mut order = Vec::new();
    if policy.return_to_primary {
        if primary_installed {
            order.push(primary);
        }
        order.append(&mut reserves);
    } else {
        order.append(&mut reserves);
        if primary_installed {
            order.push(primary);
        }
    }
    if matches!(boundary, Boundary::ForcedHandoff) {
        order.retain(|agent| Some(*agent) != current);
    }

    for agent in &order {
        match probes.get(*agent) {
            Some(Probe::Ready) | Some(Probe::Unreadable) | None => {
                return if Some(*agent) == current {
                    Next::Stay { agent: (*agent).to_owned() }
                } else {
                    Next::Start { agent: (*agent).to_owned() }
                };
            }
            Some(Probe::Limited { .. }) => {}
        }
    }

    let wake_at = order
        .iter()
        .filter_map(|agent| match probes.get(*agent) {
            Some(Probe::Limited { reset_at: Some(at), .. }) if *at > now => Some(*at),
            _ => None,
        })
        .min()
        .unwrap_or_else(|| now + Duration::minutes(1));
    Next::WaitAny { wake_at }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-22T00:00:00Z").unwrap().with_timezone(&Utc)
    }

    fn policy() -> RunFailoverSettings {
        RunFailoverSettings { enabled: true, return_to_primary: true, wait_minutes: 5, priority: vec!["claude".into(), "codex".into()] }
    }

    fn installed() -> Vec<String> {
        vec!["claude".into(), "codex".into()]
    }

    fn limited(minutes: i64) -> Probe {
        Probe::Limited { pct: 95, reset_at: Some(now() + Duration::minutes(minutes)), resets: Some(format!("in {minutes} minutes")) }
    }

    #[test]
    fn disabled_policy_keeps_waiting_for_the_primary() {
        let mut disabled = policy();
        disabled.enabled = false;
        let probes = BTreeMap::from([("claude".into(), limited(60))]);
        assert!(matches!(select(&disabled, "claude", Some("claude"), &installed(), &probes, Boundary::ForcedHandoff, now()), Next::WaitOne { agent, .. } if agent == "claude"));
    }

    #[test]
    fn short_known_reset_waits_but_long_or_unknown_reset_hands_off() {
        let short = BTreeMap::from([("claude".into(), limited(4)), ("codex".into(), Probe::Ready)]);
        assert!(matches!(select(&policy(), "claude", Some("claude"), &installed(), &short, Boundary::ForcedHandoff, now()), Next::WaitOne { .. }));
        let long = BTreeMap::from([("claude".into(), limited(6)), ("codex".into(), Probe::Ready)]);
        assert_eq!(select(&policy(), "claude", Some("claude"), &installed(), &long, Boundary::ForcedHandoff, now()), Next::Start { agent: "codex".into() });
        let unknown = BTreeMap::from([("claude".into(), Probe::Limited { pct: 95, reset_at: None, resets: None }), ("codex".into(), Probe::Ready)]);
        assert_eq!(select(&policy(), "claude", Some("claude"), &installed(), &unknown, Boundary::ForcedHandoff, now()), Next::Start { agent: "codex".into() });
    }

    #[test]
    fn project_primary_is_excluded_from_its_reserve_order() {
        let probes = BTreeMap::from([("codex".into(), limited(30)), ("claude".into(), Probe::Ready)]);
        assert_eq!(select(&policy(), "codex", Some("codex"), &installed(), &probes, Boundary::ForcedHandoff, now()), Next::Start { agent: "claude".into() });
    }

    #[test]
    fn false_return_keeps_a_healthy_reserve_but_allows_primary_after_reserve_limit() {
        let mut policy = policy();
        policy.return_to_primary = false;
        let healthy = BTreeMap::from([("claude".into(), Probe::Ready), ("codex".into(), Probe::Ready)]);
        assert_eq!(select(&policy, "claude", Some("codex"), &installed(), &healthy, Boundary::NewBatch, now()), Next::Stay { agent: "codex".into() });
        let limited_reserve = BTreeMap::from([("claude".into(), Probe::Ready), ("codex".into(), limited(60))]);
        assert_eq!(select(&policy, "claude", Some("codex"), &installed(), &limited_reserve, Boundary::ForcedHandoff, now()), Next::Start { agent: "claude".into() });
    }

    #[test]
    fn unreadable_is_a_trial_and_all_limited_wakes_at_the_earliest_known_reset() {
        let unreadable = BTreeMap::from([("claude".into(), limited(60)), ("codex".into(), Probe::Unreadable)]);
        assert_eq!(select(&policy(), "claude", Some("claude"), &installed(), &unreadable, Boundary::ForcedHandoff, now()), Next::Start { agent: "codex".into() });
        let all = BTreeMap::from([("claude".into(), limited(60)), ("codex".into(), limited(20))]);
        assert_eq!(select(&policy(), "claude", Some("claude"), &installed(), &all, Boundary::ForcedHandoff, now()), Next::WaitAny { wake_at: now() + Duration::minutes(20) });
    }
}
