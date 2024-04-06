# Weighted Rate Limiter
Some API's have usage limits where different endpoints or actions have different costs - this is often implemented as the caller having a certain amount of weight that they are allowed to use per a given time period. This crate allows you to implement a rate limiter that enforces these limits - more specifically by queueing up futures that will be executed when the weight becomes available.

See [docs.rs](https://docs.rs/weighted_rate_limiter/latest/weighted_rate_limiter/) for usage.

# To Note
This crate is very new, and has not been thoroughly tested.