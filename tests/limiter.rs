use std::time::Duration;

use weighted_rate_limiter::{Limiter, ReserveWeightFailed};

use tokio::time::{advance, Instant};

fn assert_insufficient_capacity(limiter: &Limiter, weight: u64) {
    let res = limiter.try_reserve(weight);
    assert!(res.is_err());
    match res.err().unwrap() {
        ReserveWeightFailed::InsufficientCurrentCapacity => (),
        _ => panic!("Unexpected variant"),
    };
}

fn assert_requesting_too_much(limiter: &Limiter, weight: u64) {
    let res = limiter.try_reserve(weight);
    assert!(res.is_err());
    match res.err().unwrap() {
        ReserveWeightFailed::RequestedGreaterThanTotalCapacity => (),
        _ => panic!("Unexpected variant"),
    };
}

fn elapsed(since: &Instant) -> Duration {
    Instant::now() - *since
}

/// Test basic functionality of the Limiter object.
#[tokio::test(start_paused = true)]
async fn test_basic_1() {
    let one_second = Duration::from_secs(1);
    let one_nano = Duration::from_nanos(1);

    let limiter = Limiter::new(1, Duration::from_secs(1));

    // Try reserving one weight
    assert!(limiter.try_reserve(1).is_ok());

    // Now try again - should fail
    assert_insufficient_capacity(&limiter, 1);

    // Now advance time by nearly one second
    advance(one_second - one_nano).await;
    assert_insufficient_capacity(&limiter, 1);

    // Go the rest of the way and check we can reserve 1 more (but not twice)
    advance(one_nano).await;
    assert!(limiter.try_reserve(1).is_ok());
    assert_insufficient_capacity(&limiter, 1);

    // Wait two seconds and check that we can do 1 still
    advance(2 * one_second).await;
    assert!(limiter.try_reserve(1).is_ok());
    assert_insufficient_capacity(&limiter, 1);

    // Check that we can never request too much
    advance(one_second).await;
    assert_requesting_too_much(&limiter, 2);
}

/// A slightly more involved example where we reserve (and check) capacity at different times.
#[tokio::test(start_paused = true)]
async fn test_basic_2() {
    let one_minute = Duration::from_secs(60);
    let one_second = Duration::from_secs(1);
    let one_nano = Duration::from_nanos(1);

    let start = Instant::now();

    let limiter = Limiter::new(1200, one_minute);

    // Reserve 100 at the start
    assert!(limiter.try_reserve(100).is_ok());

    // Advance 50 seconds, reserving a total of 600 by the end
    for _ in 0..5 {
        advance(10 * one_second).await;
        assert!(limiter.try_reserve(100).is_ok());
    }

    // Check that we can't reserve 601, but we can reserve 600 (the remainder)
    assert_eq!(elapsed(&start), Duration::from_secs(50));
    assert_eq!(limiter.remaining_weight(), 600);
    assert_insufficient_capacity(&limiter, 601);
    assert!(limiter.try_reserve(600).is_ok());

    // Now advance to just before when the first batch of stuff will get released.
    advance(10 * one_second - one_nano).await;
    assert_insufficient_capacity(&limiter, 1);
    assert_eq!(limiter.remaining_weight(), 0);

    // Now release the first batch of stuff
    advance(one_nano).await;
    assert_eq!(limiter.remaining_weight(), 100);
    assert!(limiter.try_reserve(100).is_ok());
    assert_eq!(limiter.remaining_weight(), 0);
    assert_eq!(elapsed(&start), Duration::from_secs(60));

    // Over the next 50 seconds, check that the other bits are released when we expect.
    // Note that there's an extra 60 released at the 50-sec mark.
    for i in 0..5 {
        advance(10 * one_second - one_nano).await;
        assert_eq!(limiter.remaining_weight(), i * 100);

        advance(one_nano).await;
        let expected = if i == 4 { 1100 } else { (i + 1) * 100 };
        assert_eq!(limiter.remaining_weight(), expected);
    }

    // And 10 seconds later we're back to full.
    advance(10 * one_second).await;
    assert_eq!(limiter.remaining_weight(), 1200);
    assert_eq!(elapsed(&start), Duration::from_secs(120));
}
