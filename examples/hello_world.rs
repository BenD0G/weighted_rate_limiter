//! Construct a limiter that can fire 5 times every 5 seconds.
//! Repeatedly try to reserve a random amount of weight.

use weighted_rate_limiter::Limiter;

use std::time::Duration;

use rand::{thread_rng, Rng};
use tokio::time::Instant;

#[tokio::main]
async fn main() {
    let limiter = Limiter::new(5, Duration::from_secs(5));

    let start = Instant::now();

    let mut rng = thread_rng();

    loop {
        let weight: u64 = rng.gen_range(1..4);
        if let Ok(()) = limiter.try_reserve(weight) {
            let now = Instant::now();
            println!("Reserved {} at {:?}", weight, now - start);
        }
    }
}
