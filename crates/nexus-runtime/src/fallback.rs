//! Resilient step execution: retry, backoff, and on-failure routing.
//!
//! Implements the workflow `fallback` semantics from the DSL — `retry N`,
//! `backoff exponential`, and `on_failure <action>` — without the developer
//! writing any imperative retry loops.

use nexus_core::Fallback;

/// What to do once all retries are exhausted.
#[derive(Clone, Debug, PartialEq)]
pub enum FailureAction {
    None,
    Route(String),
}

/// The result of running a step under a fallback policy.
#[derive(Debug)]
pub struct RetryOutcome<T> {
    pub result: Result<T, String>,
    pub attempts: u32,
    /// Total backoff time the schedule would wait (ms), computed deterministically.
    pub scheduled_wait_ms: u64,
    pub on_failure: FailureAction,
}

/// Base backoff in milliseconds for the first retry.
const BASE_BACKOFF_MS: u64 = 100;

/// Run `f` under a fallback policy. `f` receives the 1-based attempt number and
/// returns `Ok` on success or `Err(reason)` to trigger the next retry.
pub fn run_with_retry<T, F>(fb: &Fallback, mut f: F) -> RetryOutcome<T>
where
    F: FnMut(u32) -> Result<T, String>,
{
    let max_attempts = fb.retry.unwrap_or(0) + 1;
    let exponential = fb.backoff.as_deref() == Some("exponential");
    let mut scheduled_wait_ms = 0u64;
    let mut last_err = String::new();

    for attempt in 1..=max_attempts {
        match f(attempt) {
            Ok(v) => {
                return RetryOutcome {
                    result: Ok(v),
                    attempts: attempt,
                    scheduled_wait_ms,
                    on_failure: FailureAction::None,
                }
            }
            Err(e) => {
                last_err = e;
                if attempt < max_attempts {
                    scheduled_wait_ms += if exponential {
                        BASE_BACKOFF_MS * (1u64 << (attempt - 1))
                    } else {
                        BASE_BACKOFF_MS
                    };
                }
            }
        }
    }

    let on_failure = match &fb.on_failure {
        Some(action) => FailureAction::Route(action.clone()),
        None => FailureAction::None,
    };
    RetryOutcome {
        result: Err(last_err),
        attempts: max_attempts,
        scheduled_wait_ms,
        on_failure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn succeeds_first_try() {
        let fb = Fallback { retry: Some(3), backoff: None, on_failure: None };
        let out = run_with_retry(&fb, |_| Ok::<_, String>(42));
        assert_eq!(out.result.unwrap(), 42);
        assert_eq!(out.attempts, 1);
        assert_eq!(out.scheduled_wait_ms, 0);
    }

    #[test]
    fn retries_then_succeeds() {
        let fb = Fallback { retry: Some(3), backoff: Some("exponential".into()), on_failure: None };
        let out = run_with_retry(&fb, |attempt| {
            if attempt < 3 {
                Err("transient".into())
            } else {
                Ok(7)
            }
        });
        assert_eq!(out.result.unwrap(), 7);
        assert_eq!(out.attempts, 3);
        // exponential: 100 + 200 = 300ms across two failed attempts
        assert_eq!(out.scheduled_wait_ms, 300);
    }

    #[test]
    fn exhausts_and_routes_on_failure() {
        let fb = Fallback { retry: Some(2), backoff: None, on_failure: Some("DispatchAlert".into()) };
        let out: RetryOutcome<i32> = run_with_retry(&fb, |_| Err("down".into()));
        assert!(out.result.is_err());
        assert_eq!(out.attempts, 3); // 1 initial + 2 retries
        assert_eq!(out.on_failure, FailureAction::Route("DispatchAlert".into()));
    }
}
