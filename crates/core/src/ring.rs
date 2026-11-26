//! A short time window of recent items (zone crops), oldest first.

use std::collections::VecDeque;

pub struct Ring<T> {
    span_ms: f64,
    cap: usize,
    items: VecDeque<(f64, T)>,
}

impl<T> Ring<T> {
    /// Keeps items no older than `span_ms` behind the newest, and at most `cap` of them.
    pub fn new(span_ms: f64, cap: usize) -> Self {
        Ring { span_ms, cap: cap.max(1), items: VecDeque::new() }
    }

    pub fn push(&mut self, t_ms: f64, item: T) {
        self.items.push_back((t_ms, item));
        while self.items.len() > self.cap {
            self.items.pop_front();
        }
        while self.items.front().is_some_and(|(t, _)| *t < t_ms - self.span_ms) {
            self.items.pop_front();
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &(f64, T)> {
        self.items.iter()
    }

    /// The item in [from_ms, to_ms] with the highest `score` (the first one on ties).
    pub fn best_in(&self, from_ms: f64, to_ms: f64, score: impl Fn(&T) -> f32) -> Option<&(f64, T)> {
        let mut best: Option<(&(f64, T), f32)> = None;
        for e in self.items.iter().filter(|(t, _)| *t >= from_ms && *t <= to_ms) {
            let s = score(&e.1);
            if best.is_none_or(|(_, b)| s > b) {
                best = Some((e, s));
            }
        }
        best.map(|(e, _)| e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_old_items_by_time_and_count() {
        let mut r = Ring::new(100.0, 4);
        for i in 0..10 {
            r.push(i as f64 * 30.0, i);
        }
        // Newest t = 270: keep t >= 170 (i = 6..=9), and at most 4.
        assert_eq!(r.iter().map(|(_, v)| *v).collect::<Vec<_>>(), vec![6, 7, 8, 9]);
        let mut r = Ring::new(10_000.0, 3);
        for i in 0..5 {
            r.push(i as f64, i);
        }
        assert_eq!(r.len(), 3);
        r.clear();
        assert!(r.is_empty());
    }

    #[test]
    fn best_in_window() {
        let mut r = Ring::new(1000.0, 10);
        for (t, v) in [(0.0, 5.0f32), (10.0, 9.0), (20.0, 7.0), (30.0, 9.5)] {
            r.push(t, v);
        }
        assert_eq!(r.best_in(0.0, 25.0, |v| *v).map(|e| e.0), Some(10.0));
        assert_eq!(r.best_in(0.0, 40.0, |v| *v).map(|e| e.0), Some(30.0));
        assert!(r.best_in(100.0, 200.0, |v| *v).is_none());
    }
}
