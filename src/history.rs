use std::collections::VecDeque;

pub struct History {
    samples: VecDeque<u64>,
    capacity: usize,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: u64) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }

    pub fn recent(&self, n: usize) -> Vec<u64> {
        let skip = self.samples.len().saturating_sub(n);
        self.samples.iter().skip(skip).copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_oldest_when_full() {
        let mut h = History::new(3);
        for v in 1..=5 {
            h.push(v)
        }
        assert_eq!(h.recent(10), vec![3, 4, 5]);
    }

    #[test]
    fn recent_returns_newest_samples() {
        let mut h = History::new(10);
        for v in 1..=5 {
            h.push(v)
        }
        assert_eq!(h.recent(2), vec![4, 5]);
    }
}
