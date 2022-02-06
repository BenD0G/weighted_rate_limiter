use weighted_rate_limiter::RateLimiter;

use futures::join;
use std::time::Duration;
use tokio::time::Instant;

async fn make_future(x: u64) -> u64 {
    x
}

/// Test that one future can get limited.
#[tokio::test(start_paused = true)]
async fn test_basic() {
    let r = RateLimiter::new(1, Duration::from_secs(1));

    let f1 = make_future(1);

    let limited_f1 = r.rate_limit_future(f1, 1);

    let result_1 = limited_f1.await;

    assert_eq!(result_1, 1);
}

/// Test that multiple futures can get limited.
#[tokio::test(start_paused = true)]
async fn test_basic_multiple() {
    let r = RateLimiter::new(1, Duration::from_secs(1));

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
    let r = RateLimiter::new(1, Duration::from_secs(1));

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
