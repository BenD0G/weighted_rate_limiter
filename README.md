# Weighted Rate Limiter

Crate for rate-limiting, possibly with different weights for different calls (as is the case for weighted API calls).

# What we want
1. To have a queue-like structure where we can reserve some weight, with that reserved weight being returned to the available pool after a specified duration.