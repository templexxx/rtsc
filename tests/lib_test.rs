#![feature(test)]

extern crate test;

use test::Bencher;

use rtsc;

#[test]
fn tsc() {
    // assert_eq!(4, adder::add_two(2));
    let offset: f64 = 1645227926076647424.0;
    let coeff: f64 = 0.3561249436912842;
    rtsc::store_offset_coeff(offset, coeff);
    let start = rtsc::unix_nano_tsc();
    for _ in 0..100000000 {
        rtsc::unix_nano_tsc();
    }
    let end = rtsc::unix_nano_tsc();
    println!("{}ns/op", (end - start) as f64 / 100000000 as f64);
    println!("{}", start);
    println!("{}", end);
    println!("{} {}", rtsc::unix_nano_tsc(), rtsc::unix_nano_std());
}

#[bench]
fn bench_tsc(b: &mut Bencher) {
    rtsc::store_offset_coeff(1645227926083643392.0 as f64, 0.3561252045371864 as f64);

    b.iter(|| rtsc::unix_nano_tsc());
}

#[bench]
fn bench_std(b: &mut Bencher) {
    b.iter(|| rtsc::unix_nano_std());
}
