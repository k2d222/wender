use std::time::{Duration, Instant};

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

    pub fn tick(&mut self) {
        self.history[self.ptr] = Instant::now();
        self.ptr += 1;
        if self.ptr == Self::HISTORY_SIZE {
            self.ptr = 0;
        }
    }

    pub fn durations(&self) -> Vec<Duration> {
        self.history
            .windows(2)
            .map(|s| s[1].duration_since(s[0]))
            .collect()
    }
}
