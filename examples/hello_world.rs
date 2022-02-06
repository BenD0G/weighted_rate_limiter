//! Construct a limiter that can fire 5 times every 5 seconds.
//! Repeatedly try to reserve a random amount of weight.

use weighted_rate_limiter::{RateLimiter};

use std::time::Duration;

use futures::future::join_all;
use rand::{thread_rng, Rng};
use tokio::time::Instant;

async fn make_future(weight: u64, id: u64, start: &Instant) {
    println!(
        "Running job {} with weight {} at {:?}",
        id,
        weight,
        Instant::now() - *start
    );
}

#[tokio::main]
async fn main() {
    let rate_limiter = RateLimiter::new(5, Duration::from_secs(2));

    let start = Instant::now();

    let mut rng = thread_rng();

    let mut futures = vec![];

    for i in 0..1000 {
        let weight: u64 = rng.gen_range(1..4);
        let fut = make_future(weight, i, &start);
        let rate_limited_fut = rate_limiter.rate_limit_future(fut, weight);
        futures.push(Box::pin(rate_limited_fut));
    }

    join_all(futures).await;
}
