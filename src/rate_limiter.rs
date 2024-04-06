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

/// A rate limiter that enforces a weight limit over a given time period.
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

    /// Returns a future that will be executed when the weight is available.
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
