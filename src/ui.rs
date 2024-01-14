use std::time::{Duration, Instant};

use itertools::Itertools;

pub struct FpsCounter {
    history: [Instant; Self::HISTORY_SIZE],
    ptr: usize,
}

impl FpsCounter {
    const HISTORY_SIZE: usize = 64;

    pub fn new() -> Self {
        Self {
            history: [Instant::now(); Self::HISTORY_SIZE],
            ptr: 1,
        }
    }

    pub fn len(&self) -> usize {
        Self::HISTORY_SIZE
    }

    pub fn tick(&mut self) {
        self.history[self.ptr] = Instant::now();
        self.ptr += 1;
        if self.ptr == Self::HISTORY_SIZE {
            self.ptr = 0;
        }
    }

    pub fn durations(&self) -> Vec<Duration> {
        self.history[self.ptr..]
            .iter()
            .chain(self.history[0..self.ptr].iter())
            .tuple_windows()
            .map(|(s1, s2)| s2.duration_since(*s1))
            .collect()
    }
}
