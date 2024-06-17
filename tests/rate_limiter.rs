use weighted_rate_limiter::RateLimiter;

use futures::{future::join_all, join};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, Notify},
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

#[derive(Clone)]
struct JobId {
    thread_id: u64,
    job_id: u64,
}

/// A more advanced test, testing a more practical scenario: have 5 threads, each submitting some jobs with "random" weights.
/// The jobs were randomly generated, but are enforced via notify.
#[tokio::test(start_paused = true)]
async fn test_multi_threaded() {
    let rate_limiter = Arc::new(RateLimiter::new(5, Duration::from_secs(2)));
    let mut futures = vec![];

    let job_ids = vec![
        JobId {
            thread_id: 0,
            job_id: 0,
        },
        JobId {
            thread_id: 0,
            job_id: 1,
        },
        JobId {
            thread_id: 1,
            job_id: 0,
        },
        JobId {
            thread_id: 2,
            job_id: 0,
        },
        JobId {
            thread_id: 3,
            job_id: 0,
        },
        JobId {
            thread_id: 4,
            job_id: 0,
        },
        JobId {
            thread_id: 0,
            job_id: 2,
        },
        JobId {
            thread_id: 3,
            job_id: 1,
        },
        JobId {
            thread_id: 3,
            job_id: 2,
        },
        JobId {
            thread_id: 3,
            job_id: 3,
        },
        JobId {
            thread_id: 4,
            job_id: 1,
        },
        JobId {
            thread_id: 3,
            job_id: 4,
        },
        JobId {
            thread_id: 4,
            job_id: 2,
        },
    ];

    let job_ids_and_notifies = Arc::new(
        job_ids
            .into_iter()
            .map(|j| (j, Notify::new()))
            .collect::<Vec<_>>(),
    );

    let mut results = Arc::new(Mutex::new(vec![]));

    // Queue up all of the tasks immediately, but in the execution order defined by job_ids
    for thread_id in 0..5 {
        let rate_limiter = Arc::clone(&rate_limiter);
        let job_ids_and_notifies = job_ids_and_notifies.clone();
        let results = results.clone();

        let fut = task::spawn(async move {
            let mut thread_futures = vec![];
            let filtered_job_ids_and_notifies: Vec<(usize, (JobId, Notify))> = job_ids_and_notifies
                .iter()
                .enumerate()
                .filter(|(_, (job_id, x))| job_id.thread_id == thread_id)
                .map(|(_, (x, y))| (x.clone(), *y.clone()))
                .collect();

            for (i, (job_id, notify)) in filtered_job_ids_and_notifies {
                let fut = async {
                    results.lock().await.push(job_id);
                };
                // Wait for the previous task to have rate limited its future
                if i > 0 {
                    let (_, prev_notify) = &job_ids_and_notifies[i - 1];
                    prev_notify.notified().await;
                }
                let rate_limited_fut = rate_limiter.rate_limit_future(fut, 1);
                notify.notify_one();
                thread_futures.push(rate_limited_fut);
            }

            let foo = join_all(thread_futures).await;
        });

        futures.push(fut);
    }

    join_all(futures).await;
}
