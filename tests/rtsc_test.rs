#![feature(test)]

extern crate test;

use std::thread;
use std::time::Duration;
use test::Bencher;

#[test]
fn time_drift() {
    rtsc::init();
    let cnt = 256;
    let mut i = 0;
    let mut deltas: Vec<f64> = Vec::with_capacity(cnt);
    while i < cnt {
        let rt0 = rtsc::unix_nano_tsc();
        let st = rtsc::unix_nano_std();
        let rt1 = rtsc::unix_nano_tsc();
        deltas.push((rt0 + rt1) as f64 / 2.0 - st as f64);

        thread::sleep(Duration::from_micros(10));

        i += 1;
    }

    let mut delta: f64 = 0.0;
    for d in deltas {
        delta += d.abs();
    }

    assert!(
        (delta / cnt as f64) < 2000.0,
        "clock drift too big, tsc clock maybe unstable or sys clock jitter"
    );
}

#[bench]
fn bench(b: &mut Bencher) {
    rtsc::init();

    b.iter(rtsc::unix_nano);
}

#[bench]
fn bench_std(b: &mut Bencher) {
    b.iter(rtsc::unix_nano_std);
}
