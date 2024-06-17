//! Construct a limiter that can fire jobs with total weight 5 every 2 seconds.
//! Jobs are given a random amount of weight.

use weighted_rate_limiter::RateLimiter;

use std::{sync::Arc, time::Duration};

use futures::future::join_all;
use rand::{rngs::StdRng, Rng, SeedableRng};
use tokio::{task, time::Instant};

async fn make_future(weight: u64, thread: u64, id: u64, start: &Instant) {
    println!(
        "Running job {}-{} with weight {} at {:?}s",
        thread,
        id,
        weight,
        (Instant::now() - *start).as_secs()
    );
}

#[tokio::main]
async fn main() {
    let rate_limiter = Arc::new(RateLimiter::new(5, Duration::from_secs(2)));

    let start = Instant::now();

    let mut futures = vec![];

    for thread_id in 0..10 {
        let rate_limiter = Arc::clone(&rate_limiter);
        let start = start.clone();

        let fut = task::spawn(async move {
            let mut rng = StdRng::seed_from_u64(69);
            let mut thread_futures = vec![];
            for job_id in 0..100 {
                let weight: u64 = rng.gen_range(1..4);
                let fut = make_future(weight, thread_id, job_id, &start);
                let rate_limited_fut = rate_limiter.rate_limit_future(fut, weight);
                thread_futures.push(rate_limited_fut);
            }
            join_all(thread_futures).await;
        });

        futures.push(fut);
    }

    join_all(futures).await;
}
