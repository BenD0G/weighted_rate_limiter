use std::time::Duration;

use weighted_rate_limiter::{Limiter, ReserveWeightFailed};

use tokio::time::advance;

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

#[tokio::test(start_paused = true)]
async fn test_basic() {
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
