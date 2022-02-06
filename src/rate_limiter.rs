//! Provides a struct for rate-limiting Future executions.
//!
//! The basic flow is:
//! 1. Pass your Future into the machine
//! 2. This is added to a queue and waits to be processed
//! 3. A future is returned that, when polled:
//!     a) If the underlying future is pending, returns pending ("I'm working on it, ok??")
//!     b) If waiting,
//!
//!
//! Notes for next time: expose another bit of API on the Limiter that returns the amount of time
//! until the next blcok of weight will expire.
//! That way you can creat a sleep() here that depends on that and might blow this whole async malarkey wide open.

use crate::Limiter;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use tokio::time::{sleep_until, Sleep};

static COUNTER: AtomicU64 = AtomicU64::new(1);

struct QueueData {
    job_id: u64,
}

pub struct RateLimiter {
    limiter: Limiter,
    queued_jobs: RefCell<VecDeque<QueueData>>,
}

impl RateLimiter {
    pub fn new(limiter: Limiter) -> Self {
        Self {
            limiter,
            queued_jobs: RefCell::new(VecDeque::new()),
        }
    }

    fn make_id() -> u64 {
        COUNTER.fetch_add(1, Relaxed)
    }

    fn wait_until_weight_is_released(&self) -> Option<Sleep> {
        match self.limiter.time_of_next_weight_released() {
            Some(instant) => Some(sleep_until(instant)),
            None => None,
        }
    }

    pub async fn rate_limit_future<T, F>(&self, future: F, weight: u64) -> T
    where
        F: Future<Output = T>,
    {
        // Add job to queue
        let job_id = Self::make_id();
        let queue_data = QueueData { job_id };
        self.queued_jobs.borrow_mut().push_back(queue_data);

        while self
            .queued_jobs
            .borrow()
            .front()
            .expect("queued_jobs was empty - shouldn't happen!")
            .job_id
            != job_id
        {
            self.wait_until_weight_is_released()
                .expect("wait_until_weight_is_released is None")
                .await;
        }

        // Our job is now next in line. Try to reserve the weight - if it fails, wait one more turn.
        if self.limiter.try_reserve(weight).is_err() {
            self.wait_until_weight_is_released()
                .expect("wait_until_weight_is_released is None")
                .await;
            if self.limiter.try_reserve(weight).is_err() {
                panic!("Failed final weight reservation")
            }
        }
        self.queued_jobs.borrow_mut().pop_front();

        future.await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::join;
    use std::time::Duration;
    use tokio::time::Instant;

    async fn make_future(x: u64) -> u64 {
        x
    }

    /// Test that one future can get limited.
    #[tokio::test(start_paused = true)]
    async fn test_basic() {
        let r = RateLimiter::new(Limiter::new(1, Duration::from_secs(1)));

        let f1 = make_future(1);

        let limited_f1 = r.rate_limit_future(f1, 1);

        let result_1 = limited_f1.await;

        assert_eq!(result_1, 1);
    }

    /// Test that multiple futures can get limited.
    #[tokio::test(start_paused = true)]
    async fn test_basic_multiple() {
        let r = RateLimiter::new(Limiter::new(1, Duration::from_secs(1)));

        let start = Instant::now();

        let (f1, f2, f3) = (make_future(1), make_future(2), make_future(3));

        let (limited_f1, limited_f2, limited_f3) = (
            r.rate_limit_future(f1, 1),
            r.rate_limit_future(f2, 1),
            r.rate_limit_future(f3, 1),
        );

        let result_1 = limited_f1.await;
        assert_eq!(Instant::now() - start, Duration::from_secs(0)); // Can fire straight away
        let result_2 = limited_f2.await;
        assert_eq!(Instant::now() - start, Duration::from_secs(1)); // Need to wait a second before firing
        let result_3 = limited_f3.await;
        assert_eq!(Instant::now() - start, Duration::from_secs(2)); // Wait another second

        assert_eq!(result_1, 1);
        assert_eq!(result_2, 2);
        assert_eq!(result_3, 3);
    }

    /// Test that we can await several futures at the same time
    #[tokio::test(start_paused = true)]
    async fn test_multiple_concurrent() {
        let r = RateLimiter::new(Limiter::new(1, Duration::from_secs(1)));

        let start = Instant::now();

        let (f1, f2, f3) = (make_future(1), make_future(2), make_future(3));

        let (limited_f1, limited_f2, limited_f3) = (
            r.rate_limit_future(f1, 1),
            r.rate_limit_future(f2, 1),
            r.rate_limit_future(f3, 1),
        );

        let results = join!(limited_f1, limited_f2, limited_f3);

        assert_eq!(results.0, 1);
        assert_eq!(results.1, 2);
        assert_eq!(results.2, 3);

        assert_eq!(Instant::now() - start, Duration::from_secs(2)); // We must have waited 2 seconds to fire 3 things
    }
}
