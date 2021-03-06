#![feature(test)]
extern crate test; // Required for testing, even though extern crate is no longer needed in the 2018 version, this is a special case

use std::sync::mpsc;
use test::{black_box, Bencher};

#[bench]
fn channel_sending_enum(b: &mut Bencher) {
    enum Test {
        A,
    }
    let (tx, rx) = mpsc::sync_channel(1);
    b.iter(|| {
        tx.send(black_box(Test::A)).unwrap();
        rx.recv().unwrap();
    });
}

#[bench]
fn channel_sending_fn(b: &mut Bencher) {
    let (tx, rx) = mpsc::sync_channel(1);
    b.iter(|| {
        tx.send(black_box(|| {})).unwrap();
        rx.recv().unwrap();
    });
}
