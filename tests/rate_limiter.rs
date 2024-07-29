use weighted_rate_limiter::RateLimiter;

use futures::{future::join_all, join};
use pretty_assertions::assert_eq;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{Barrier, Mutex},
    task,
    time::Instant,
};

async fn make_future<T>(x: T) -> T {
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

/// Have 5 tasks, each submitting 3 jobs. All jobs will have weight 1. We will use barriers such that
/// the jobs are submitted in batches; one from each thread. We want to check that the number of jobs executed
/// each second is as expected.
#[tokio::test(start_paused = true)]
async fn test_multi_tasked() {
    let rate_limiter = Arc::new(RateLimiter::new(3, Duration::from_secs(1)));
    let num_threads = 5;
    let num_batches = 3;

    let barriers = Arc::new(vec![
        Barrier::new(num_threads),
        Barrier::new(num_threads),
        Barrier::new(num_threads),
    ]);

    let second_to_result_count = Arc::new(Mutex::new(HashMap::new()));

    let start = Instant::now();

    let mut futures = Vec::new();

    // Queue up all of the tasks immediately, but in the execution order defined by job_ids
    for _ in 0..num_threads {
        let rate_limiter = Arc::clone(&rate_limiter);
        let barriers = Arc::clone(&barriers);
        let second_to_result_count = Arc::clone(&second_to_result_count);

        let fut = task::spawn(async move {
            for batch in 0..num_batches {
                // Just record when this is evaluated
                let fut = async {
                    let duration: u64 = (Instant::now() - start).as_secs();
                    second_to_result_count
                        .lock()
                        .await
                        .entry(duration)
                        .and_modify(|x| *x += 1)
                        .or_insert(1);
                };

                // Wait until all threads are ready to submit their jobs.
                barriers[batch].wait().await;
                rate_limiter.rate_limit_future(fut, 1).await;
            }
        });

        futures.push(fut);
    }

    // Just make sure they're all finished.
    join_all(futures).await;

    let expected = HashMap::from([(0, 3), (1, 3), (2, 3), (3, 3), (4, 3)]);

    assert_eq!(*second_to_result_count.lock().await, expected);
}

/// Have 5 threads, each submitting 3 jobs. All jobs will have weight 1. We will use barriers such that
/// the jobs are submitted in batches; one from each thread. We want to check that the number of jobs executed
/// each second is as expected.
#[tokio::test(flavor = "multi_thread")]
async fn test_multi_threaded() {
    let rate_limiter = Arc::new(RateLimiter::new(3, Duration::from_secs(1)));
    let num_threads = 5;
    let num_batches = 3;

    let barriers = Arc::new(vec![
        Barrier::new(num_threads),
        Barrier::new(num_threads),
        Barrier::new(num_threads),
    ]);

    let second_to_result_count = Arc::new(Mutex::new(HashMap::new()));

    let start = Instant::now();

    let mut futures = Vec::new();

    // Queue up all of the tasks immediately, but in the execution order defined by job_ids
    for _ in 0..num_threads {
        let rate_limiter = Arc::clone(&rate_limiter);
        let barriers = Arc::clone(&barriers);
        let second_to_result_count = Arc::clone(&second_to_result_count);

        let fut = task::spawn(async move {
            for batch in 0..num_batches {
                // Just record when this is evaluated
                let fut = async {
                    let duration: u64 = (Instant::now() - start).as_secs();
                    second_to_result_count
                        .lock()
                        .await
                        .entry(duration)
                        .and_modify(|x| *x += 1)
                        .or_insert(1);
                };

                // Wait until all threads are ready to submit their jobs.
                barriers[batch].wait().await;
                rate_limiter.rate_limit_future(fut, 1).await;
            }
        });

        futures.push(fut);
    }

    // Just make sure they're all finished.
    join_all(futures).await;

    let expected = HashMap::from([(0, 3), (1, 3), (2, 3), (3, 3), (4, 3)]);

    assert_eq!(*second_to_result_count.lock().await, expected);
}

/// Have 2 threads, each submitting 1 job. The capacity is 3, so we want to be sure that both jobs run immediately.
#[tokio::test(flavor = "multi_thread")]
async fn test_multi_threaded_simple() {
    let rate_limiter = Arc::new(RateLimiter::new(3, Duration::from_secs(1)));
    let num_threads = 2;

    let second_to_result_count = Arc::new(Mutex::new(HashMap::new()));

    let start = Instant::now();

    let mut futures = Vec::new();

    // Queue up all of the tasks immediately, but in the execution order defined by job_ids
    for _ in 0..num_threads {
        let rate_limiter = Arc::clone(&rate_limiter);
        let second_to_result_count = Arc::clone(&second_to_result_count);

        let fut = task::spawn(async move {
            // Just record when this is evaluated
            let fut = async {
                let duration: u64 = (Instant::now() - start).as_secs();
                second_to_result_count
                    .lock()
                    .await
                    .entry(duration)
                    .and_modify(|x| *x += 1)
                    .or_insert(1);
            };

            rate_limiter.rate_limit_future(fut, 1).await;
        });

        futures.push(fut);
    }

    // Just make sure they're all finished.
    join_all(futures).await;

    let expected = HashMap::from([(0, 2)]);

    assert_eq!(*second_to_result_count.lock().await, expected);
}
