use crate::WeightManager;

use std::collections::VecDeque;
use std::future::Future;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{sleep_until, Sleep};

static COUNTER: AtomicU64 = AtomicU64::new(1);

struct QueueData {
    job_id: u64,
}

/// A rate limiter that enforces a weight limit over a given time period.
pub struct RateLimiter {
    limiter: WeightManager,
    queued_jobs: Arc<Mutex<VecDeque<QueueData>>>,
}

impl RateLimiter {
    pub fn new(max_weight_per_duration: u64, duration: Duration) -> Self {
        Self {
            limiter: WeightManager::new(max_weight_per_duration, duration),
            queued_jobs: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn make_id() -> u64 {
        COUNTER.fetch_add(1, Ordering::SeqCst)
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
        let mut next_job_id;
        {
            let mut queue = self.queued_jobs.lock().await;
            queue.push_back(queue_data);
            next_job_id = queue
                .front()
                .expect("queued_jobs was empty - shouldn't happen!")
                .job_id;
        }

        while next_job_id != job_id {
            // If we would need to wait anyway, do some waiting.
            if self.limiter.remaining_weight() < weight {
                if let Some(fut) = self.wait_until_weight_is_released() {
                    fut.await;
                }
            }

            {
                let queue = self.queued_jobs.lock().await;
                next_job_id = queue
                    .front()
                    .expect("queued_jobs was empty - shouldn't happen!")
                    .job_id;
            }
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
        {
            let mut queue = self.queued_jobs.lock().await;
            queue.pop_front();
        }

        future.await
    }
}
