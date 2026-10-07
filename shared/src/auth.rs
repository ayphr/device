/// Default number of failed attempts allowed before a lockout begins.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;
/// Default lockout duration after too many failed attempts, in milliseconds.
pub const DEFAULT_LOCKOUT_MS: u64 = 60_000;

/// Tracks consecutive authentication failures and locks the caller out once
/// too many failures happen in a row.
///
/// Time is supplied by the caller as milliseconds from any monotonically
/// increasing epoch, which keeps this type `no_std` friendly and trivial to
/// test.
#[derive(Debug, Clone)]
pub struct AuthRateLimiter {
    max_attempts: u32,
    lockout_ms: u64,
    failed_attempts: u32,
    locked_until_ms: Option<u64>,
}

impl AuthRateLimiter {
    pub const fn new(max_attempts: u32, lockout_ms: u64) -> Self {
        Self {
            max_attempts: if max_attempts == 0 { 1 } else { max_attempts },
            lockout_ms,
            failed_attempts: 0,
            locked_until_ms: None,
        }
    }

    /// Limiter using [`DEFAULT_MAX_ATTEMPTS`] and [`DEFAULT_LOCKOUT_MS`].
    pub const fn with_defaults() -> Self {
        Self::new(DEFAULT_MAX_ATTEMPTS, DEFAULT_LOCKOUT_MS)
    }

    /// Whether authentication is currently refused because of a lockout.
    pub fn is_locked(&mut self, now_ms: u64) -> bool {
        match self.locked_until_ms {
            Some(deadline) if now_ms < deadline => true,
            Some(_) => {
                self.locked_until_ms = None;
                false
            }
            None => false,
        }
    }

    /// Remaining lockout time in milliseconds (0 when not locked).
    pub fn remaining_lockout_ms(&self, now_ms: u64) -> u64 {
        self.locked_until_ms
            .map(|deadline| deadline.saturating_sub(now_ms))
            .unwrap_or(0)
    }

    /// Records a failed attempt. Returns whether this failure started a
    /// lockout.
    pub fn record_failure(&mut self, now_ms: u64) -> bool {
        if self.is_locked(now_ms) {
            return false;
        }

        self.failed_attempts = self.failed_attempts.saturating_add(1);
        if self.failed_attempts < self.max_attempts {
            return false;
        }

        self.failed_attempts = 0;
        self.locked_until_ms = Some(now_ms.saturating_add(self.lockout_ms));
        true
    }

    /// Clears failures and any lockout, e.g. after a successful login.
    pub fn reset(&mut self) {
        self.failed_attempts = 0;
        self.locked_until_ms = None;
    }

    /// Consecutive failures recorded so far (0 while locked).
    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_attempts_below_the_threshold() {
        let mut limiter = AuthRateLimiter::new(3, 1_000);

        assert!(!limiter.record_failure(0));
        assert!(!limiter.record_failure(10));
        assert!(!limiter.is_locked(20));
        assert_eq!(limiter.failed_attempts(), 2);
    }

    #[test]
    fn locks_out_after_max_failures() {
        let mut limiter = AuthRateLimiter::new(3, 1_000);

        assert!(!limiter.record_failure(0));
        assert!(!limiter.record_failure(10));
        assert!(limiter.record_failure(20), "third failure triggers lockout");

        assert!(limiter.is_locked(21));
        assert!(limiter.is_locked(1_019));
        assert_eq!(limiter.remaining_lockout_ms(520), 500);
    }

    #[test]
    fn lockout_expires_and_allows_new_tries() {
        let mut limiter = AuthRateLimiter::new(2, 500);

        assert!(!limiter.record_failure(0));
        assert!(limiter.record_failure(10));

        assert!(limiter.is_locked(400));
        assert!(!limiter.is_locked(510), "lockout expired");
        assert!(!limiter.is_locked(600), "expiry is remembered as cleared");
        assert_eq!(limiter.failed_attempts(), 0);
        assert!(!limiter.record_failure(600));
        assert!(limiter.record_failure(700), "fresh cycle still locks");
    }

    #[test]
    fn success_resets_the_counter_and_lockout() {
        let mut limiter = AuthRateLimiter::new(2, 1_000);

        assert!(!limiter.record_failure(0));
        assert!(limiter.record_failure(10));
        limiter.reset();

        assert!(!limiter.is_locked(20));
        assert_eq!(limiter.failed_attempts(), 0);
        assert_eq!(limiter.remaining_lockout_ms(20), 0);
        assert!(!limiter.record_failure(30));
        assert!(limiter.record_failure(40));
    }

    #[test]
    fn failures_while_locked_do_not_extend_the_lockout() {
        let mut limiter = AuthRateLimiter::new(1, 1_000);

        assert!(limiter.record_failure(0));
        assert!(!limiter.record_failure(500));
        assert!(limiter.is_locked(999));
        assert_eq!(limiter.remaining_lockout_ms(999), 1);
    }

    #[test]
    fn max_attempts_of_zero_is_clamped_to_one() {
        let mut limiter = AuthRateLimiter::new(0, 100);
        assert!(limiter.record_failure(0));
        assert!(limiter.is_locked(1));
    }

    #[test]
    fn defaults_are_sane() {
        let mut limiter = AuthRateLimiter::with_defaults();
        for attempt in 0..DEFAULT_MAX_ATTEMPTS - 1 {
            assert!(!limiter.record_failure(u64::from(attempt)));
        }
        assert!(limiter.record_failure(u64::from(DEFAULT_MAX_ATTEMPTS - 1)));
        assert!(limiter.is_locked(u64::from(DEFAULT_MAX_ATTEMPTS)));
        assert_eq!(
            limiter.remaining_lockout_ms(u64::from(DEFAULT_MAX_ATTEMPTS - 1)),
            DEFAULT_LOCKOUT_MS
        );
    }
}
