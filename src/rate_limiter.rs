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

use crate::WeightManager;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;

use tokio::time::{sleep_until, Sleep};

static COUNTER: AtomicU64 = AtomicU64::new(1);

struct QueueData {
    job_id: u64,
}

pub struct RateLimiter {
    limiter: WeightManager,
    queued_jobs: RefCell<VecDeque<QueueData>>,
}

impl RateLimiter {
    pub fn new(max_weight_per_duration: u64, duration: Duration) -> Self {
        Self {
            limiter: WeightManager::new(max_weight_per_duration, duration),
            queued_jobs: RefCell::new(VecDeque::new()),
        }
    }

    fn make_id() -> u64 {
        COUNTER.fetch_add(1, Relaxed)
    }

    fn wait_until_weight_is_released(&self) -> Option<Sleep> {
        self.limiter.time_of_next_weight_released().map(sleep_until)
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
