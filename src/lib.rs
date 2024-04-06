//! # Weighted Rate Limiter
//! Some API's have usage limits where different endpoints or actions have different costs - this is often implemented as the caller having a certain amount of weight that they are allowed to use per a given time period. This crate allows you to implement a rate limiter that enforces these limits - more specifically by queueing up futures that will be executed when the weight becomes available.
//!
//! # Example
//! Here, we create a rate limiter that allows a total weight of 2 every 1 second. We then create two futures: one that returns 1 and one that returns "Hello" (note the different return types). We then rate limit these futures with weights 1 and 2 respectively. The first future will be executed immediately, while the second will be executed after 1 second, when the weight allowance has refreshed.
//! ```
//! # use tokio::time::Instant;
//! # use weighted_rate_limiter::RateLimiter;
//! # use std::time::Duration;
//! # use futures::join;
//! # #[tokio::main]
//! # async fn main() {
//!     let rate_limiter = RateLimiter::new(2, Duration::from_secs(1));
//!     let start = Instant::now();
//!     let fut1 = async { println!("Returning 1 at {:?}", Instant::now() - start); 1 };
//!     let fut2 = async { println!("Returning 'Hello' at {:?}", Instant::now() - start); "Hello!" };
//!     let rate_limited_fut1 = rate_limiter.rate_limit_future(fut1, 1);
//!     let rate_limited_fut2 = rate_limiter.rate_limit_future(fut2, 2);
//!     println!("{:?}", join!(rate_limited_fut1, rate_limited_fut2));
//!
//!     // Returning 1 at 5.2µs
//!     // Returning 'Hello' at 1.0004935s
//!     // (1, "Hello!")
//! # }
//! ```
//!
//! # Limitations
//! This crate is very new, and has not been tested in production.
//!
//! Futures are executed in the order they are rate limited, not in the order they are created.

mod rate_limiter;
mod weight_manager;

pub use rate_limiter::RateLimiter;
pub(crate) use weight_manager::WeightManager;
