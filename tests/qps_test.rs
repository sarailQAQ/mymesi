use mymesi::*;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Distribution, Normal};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Instant;

#[test]
fn qps_test() {
    let n = 1;
    let round = 60000;

    let bus_line = Arc::new(Mutex::new(BusLine::new("./data/db")));
    let barrier = Arc::new(Barrier::new(n));

    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        let b = barrier.clone();
        let idx = i.clone();
        let bl = bus_line.clone();

        let handle = thread::spawn(move || {
            let mut ct = CacheController::new(bl, "".to_string());
            let normal: Normal<f64> = Normal::new(0.0, 30.0).unwrap();

            b.wait();

            let start = Instant::now();
            for i in 0..round {
                let key = normal
                    .sample(&mut rand::thread_rng())
                    .abs()
                    .floor()
                    .to_i64()
                    .unwrap()
                    .to_string();
                if idx % 2 == 1 {
                    ct.set(key, (idx * 10 + i).to_string());
                } else {
                    ct.get(key);
                }
            }
            let end = start.elapsed();
            println!(
                "thread {:?} time cost: {:?} ms, QPS is {:?},\n\
                average time cost per opt is {:?}",
                idx,
                end.as_millis(),
                ((round as f64) / end.as_secs_f64()) as i64,
                end.as_secs_f64() / (round as f64),
            );
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap()
    }
}
