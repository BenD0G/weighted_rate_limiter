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
use std::collections::{HashSet, VecDeque};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::task::Poll;
use std::time::Duration;
use std::{future::Future, rc::Rc};

use futures::future::BoxFuture;
use futures::FutureExt;
use pin_project::pin_project;
use queues::{IsQueue, Queue};
use tokio::time::{sleep, Instant, Sleep};

static COUNTER: AtomicU64 = AtomicU64::new(1);

type PinnedFuture<T> = Pin<Box<dyn Future<Output = T>>>;
type BoxedFuture<T> = Box<dyn Future<Output = T>>;

#[pin_project]
struct RateLimitedFutureOld<T, F>
where
    F: Fn(u64) -> bool,
{
    #[pin]
    inner_future: PinnedFuture<T>,
    #[pin]
    is_ready: F,
    id: u64,
}

#[pin_project]
struct RateLimitedFuture<'a, T> {
    #[pin]
    inner_future: PinnedFuture<T>,
    rate_limiter: &'a RateLimiter,
    green_light_go: bool,
    job_id: u64,
}

impl<'a, T> Future for RateLimitedFuture<'a, T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // If we've previously been told that we are allowed to start, just delegate to the inner task
        if *this.green_light_go {
            return match this.inner_future.poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(x) => {
                    this.rate_limiter.remove_id(this.job_id.clone());
                    Poll::Ready(x)
                }
            };
        }

        // If we _hadn't_ been told that we are ready, but we are now...
        if this.rate_limiter.is_ready(this.job_id.clone()) {
            // Save our state so we don't have to query this again.
            *this.green_light_go = true;
            return match this.inner_future.poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(x) => {
                    this.rate_limiter.remove_id(this.job_id.clone());
                    Poll::Ready(x)
                }
            };
        } else {
            // Wait for the sleep
        }

        match this.inner_future.poll(cx) {
            Poll::Pending => {}
            Poll::Ready(x) => return Poll::Ready(x),
        }

        //
        todo!()
    }
}

struct QueueData {
    id: u64,
    weight: u64,
}

struct RateLimiter {
    limiter: Limiter,
    queued_jobs: RefCell<VecDeque<QueueData>>,
    ready_job_ids: RefCell<HashSet<u64>>,
    next_time_weight_is_released: Option<Instant>,
}

impl RateLimiter {
    fn make_id() -> u64 {
        COUNTER.fetch_add(1, Relaxed)
    }

    fn is_ready(&self, id: u64) -> bool {
        self.ready_job_ids.borrow().contains(&id)
    }

    fn remove_id(&self, id: u64) -> bool {
        self.ready_job_ids.borrow_mut().remove(&id)
    }

    fn wait_until_weight_is_released(&self) -> Option<Sleep> {
        match self.limiter.time_of_next_weight_released() {
            Some(instant) => Some(tokio::time::sleep_until(instant)),
            None => None,
        }
    }

    fn transform_future<T, IsReady>(
        &self,
        f: PinnedFuture<T>,
        weight: u64,
    ) -> RateLimitedFuture<T> {
        let job_id = Self::make_id();
        let queue_data = QueueData { weight, id: job_id };
        let mut queue = self.queued_jobs.borrow_mut();
        queue.push_back(queue_data);

        RateLimitedFuture {
            inner_future: f,
            rate_limiter: &self,
            green_light_go: false,
            job_id,
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use futures::future::join;
//     use std::time::Duration;
//     use tokio::time::Instant;

//     #[tokio::test(start_paused = true)]
//     async fn poop() {
//         let r = RateLimiter {
//             limiter: Limiter::new(1, Duration::from_secs(1)),
//         };

//         let start = Instant::now();

//         let actual_future = async {
//             println!("fuck");
//             1
//         };

//         let counting_future = async {
//             for _ in 0..5 {
//                 sleep(Duration::from_secs(1)).await;
//                 println!("Time is {:?}", Instant::now() - start);
//             }
//             3
//         };

//         let bar = r.transform_future(actual_future);

//         let wrapped_bar = async { bar.await };

//         let combined_future = join(counting_future, wrapped_bar);

//         let baz: (i32, i32) = combined_future.await;
//         assert_eq!(baz, (3, 1));

//         panic!("poop")
//     }
