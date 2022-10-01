#![feature(bench_black_box)]

use std::hint::black_box;
use std::time::{Duration, Instant};

const WARMUP: u32 = 100 * 100;
const ITERATIONS: u32 = 1000 * 100;

fn main() {
    //let e = include_bytes!("../../lz4_Block_format.md.lz4b").as_slice();
    let e = std::fs::read("../lz4_Block_format.md.lz4b").unwrap();
    let l = 10683;
    //let e = std::fs::read("../rdfsbase.iso.lz4b").unwrap();
    //let l = 2097152;

    let d = measure(
        "nora_lz4",
        || nora_lz4::block::decompress(&e, l).unwrap(),
        l,
    );
    std::fs::write("/tmp/rdfsbase3.iso", &d).unwrap();

    let d = measure(
        "lz4_flex",
        || lz4_flex::block::decompress(&e, l).unwrap(),
        l,
    );
    std::fs::write("/tmp/rdfsbase2.iso", &d).unwrap();
}

fn measure<R: Default>(p: &str, f: impl Fn() -> R, l: usize) -> R {
    for _ in 0..WARMUP {
        black_box(f());
    }

    let mut dt = Duration::ZERO;
    let mut d = Default::default();
    for _ in 0..ITERATIONS {
        let t = std::time::Instant::now();
        d = black_box(f());
        dt += Instant::now().duration_since(t);
    }
    dt /= ITERATIONS;
    let s = (l as u128 * 1000_000_000 / dt.as_nanos()) as f64 / 1000_000_000.;
    println!("{}: {:?}, {} GB/s", p, dt, s);
    d
}
