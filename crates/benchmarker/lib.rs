//! Simple, stupid benchmarker
//!
//! This crate provides two benchmarkers, one stopwatch for single-case benchmarking, and another
//! for collecting many samples.
//!
//! ```
//! use benchmarker::stopwatch;
//!
//! fn main() {
//!     stopwatch(|| {
//!         // Operation taking time
//!     }, |t: std::time::Duration| {
//!         println!["It took {:?}", t];
//!     });
//! }
//! ```
//!
//! Allows simple sums of repeated benchmarks. Does no statistical processing,
//! just adds N benchmarks together before returning the sum from `stop`.
//!
//! ```
//! use benchmarker::Benchmarker;
//!
//! fn main() {
//!     // Setup, with 0 buffered benchmarks
//!     let mut bench = Benchmarker::new(0);
//!
//!     // Start the benchmark
//!     bench.start();
//!
//!     // --- Do some stuff
//!
//!     // Stop the benchmark, if this is the end of summing, this call
//!     // will return a Some(Duration), otherwise None.
//!     assert![bench.stop().is_some()];
//! }
//! ```
//!
//! We may also be interested in multiple samples just because.
//!
//! ```
//! use benchmarker::Benchmarker;
//!
//! fn main() {
//!     // Setup, with 99 buffered benchmarks
//!     let mut bench = Benchmarker::new(100);
//!
//!     for _ in 0..99 {
//!         // Start the benchmark
//!         bench.start();
//!
//!         // --- Do some stuff
//!
//!         // Stop the benchmark, if this is the end of summing, this call
//!         // will return a Some(Duration), otherwise None.
//!         assert![bench.stop().is_none()];
//!     }
//!
//!     // Do the final benchmark
//!     bench.start();
//!     assert![bench.stop().is_some()];
//! }
//! ```
//!
//! Note that with 99 buffers, the final call to `stop` will return a duration
//! sum representing 99+1 samples. This is to allow the 0-case, where we have
//! 0+1 samples.
#![feature(test)]
use std::time::{Duration, Instant};

extern crate test;

/// Run a stopwatch on a function, use a function to report said time
pub fn stopwatch<T: FnMut(), R: FnMut(std::time::Duration)>(mut timer: T, mut reporter: R) {
    let before = Instant::now();
    timer();
    let after = Instant::now();
    reporter(after - before);
}

pub struct Benchmarker {
    last: Instant,
    count: usize,
    window: usize,
    sum: Duration,
}

impl Benchmarker {
    pub fn new(window: usize) -> Benchmarker {
        Benchmarker {
            last: Instant::now(),
            count: 0,
            window,
            sum: Duration::new(0, 0),
        }
    }

    pub fn start(&mut self) {
        self.last = Instant::now();
    }

    pub fn stop(&mut self) -> Option<Duration> {
        let now = Instant::now();
        self.sum += now - self.last;
        self.count += 1;
        if self.count >= self.window {
            let ret = Some(self.sum / self.count as u32);
            self.count = 0;
            self.sum = Duration::new(0, 0);
            ret
        } else {
            None
        }
    }

    pub fn run<T>(&mut self, mut f: impl FnMut() -> T) -> (T, Option<Duration>) {
        self.start();
        let t = f();
        (t, self.stop())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::{black_box, Bencher};

    #[test]
    fn zero_length() {
        let mut ben = Benchmarker::new(0);
        ben.start();
        assert![ben.stop().is_some()];
        ben.start();
        assert![ben.stop().is_some()];
    }

    #[test]
    fn basic_length() {
        let mut ben = Benchmarker::new(1);
        ben.start();
        assert![ben.stop().is_some()];
        ben.start();
        assert![ben.stop().is_some()];
    }

    #[test]
    fn basic_length_run() {
        let mut ben = Benchmarker::new(1);
        let ((), duration) = ben.run(|| {});
        assert![duration.is_some()];
    }

    #[test]
    fn five_length() {
        let mut ben = Benchmarker::new(5);
        ben.start();
        assert![ben.stop().is_none()];
        ben.start();
        assert![ben.stop().is_none()];
        ben.start();
        assert![ben.stop().is_none()];
        ben.start();
        assert![ben.stop().is_none()];
        ben.start();
        assert![ben.stop().is_some()];
    }

    // ---

    #[bench]
    fn zero_usage(b: &mut Bencher) {
        let mut ben = Benchmarker::new(0);
        b.iter(|| {
            ben.start();
            black_box(ben.stop());
        });
    }

    #[bench]
    fn casual_usage(b: &mut Bencher) {
        let mut ben = Benchmarker::new(100);
        b.iter(|| {
            ben.start();
            black_box(ben.stop());
        });
    }

    #[bench]
    fn large_usage(b: &mut Bencher) {
        let mut ben = Benchmarker::new(1_000_000_000);
        b.iter(|| {
            ben.start();
            black_box(ben.stop());
        });
    }
}
