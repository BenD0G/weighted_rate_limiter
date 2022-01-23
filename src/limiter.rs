use std::cell::RefCell;
use std::time::Duration;

use queues::{IsQueue, Queue};
use tokio::time::Instant;

pub enum ReserveWeightFailed {
    RequestedGreaterThanTotalCapacity,
    InsufficientCurrentCapacity,
}

/// An amount of weight reserved, and the time at which to relinquish this back to the pool.
#[derive(Clone)]
struct WeightReservation {
    reserved_weight: u64,
    time_to_release_weight: Instant,
}

pub struct Limiter {
    duration: Duration,
    maximum_capacity: u64,
    remaining_weight: RefCell<u64>,
    reserved_weights: RefCell<Queue<WeightReservation>>,
}

impl Limiter {
    pub fn new(max_weight_per_duration: u64, duration: Duration) -> Self {
        Self {
            duration: duration,
            maximum_capacity: max_weight_per_duration,
            remaining_weight: RefCell::new(max_weight_per_duration),
            reserved_weights: RefCell::new(Queue::new()),
        }
    }

    /// Remove the weight from our remaining capacity, and add this reservation to our queue.
    fn reserve(&self, now: &Instant, weight: u64) {
        *self.remaining_weight.borrow_mut() -= weight;
        let mut queue = self.reserved_weights.borrow_mut();
        queue
            .add(WeightReservation {
                reserved_weight: weight,
                time_to_release_weight: now
                    .checked_add(self.duration)
                    .unwrap_or_else(|| panic!("Could not add {:?} to {:?}", now, self.duration)),
            })
            .expect("Queue is full");
    }

    /// Iterate through the queue and release any expired weight reservations back to the pool.
    /// The queued items are assumed to have non-decreasing times, so we can break early.
    fn release_weight(&self, now: &Instant) {
        let mut queue = self.reserved_weights.borrow_mut();
        loop {
            match queue.peek() {
                Ok(weight_reservation) => {
                    if &weight_reservation.time_to_release_weight <= now {
                        *self.remaining_weight.borrow_mut() += weight_reservation.reserved_weight;
                        queue.remove().unwrap();
                    } else {
                        break; // The next item to be released is still in the future
                    }
                }
                _ => break, // Queue is empty; nothing to do
            }
        }
    }

    /// Attempt to reserve an amount of weight.
    pub fn try_reserve(&self, weight: u64) -> Result<(), ReserveWeightFailed> {
        if weight > self.maximum_capacity {
            return Err(ReserveWeightFailed::RequestedGreaterThanTotalCapacity);
        }

        // Save this so that we have a consistent view of time across the following method calls.
        let now = Instant::now();

        self.release_weight(&now);

        if weight <= *self.remaining_weight.borrow() {
            self.reserve(&now, weight);
            Ok(())
        } else {
            Err(ReserveWeightFailed::InsufficientCurrentCapacity)
        }
    }

    /// Release any expired weight reservations back to the total pool and return the total remaining.
    pub fn remaining_weight(&self) -> u64 {
        let now = Instant::now();
        self.release_weight(&now);
        *self.remaining_weight.borrow()
    }
}
