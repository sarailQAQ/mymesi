use mymesi::*;
use parking_lot::RwLock;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Distribution, Normal};
use std::ops::Add;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Instant;

#[test]
fn qps_test() {
    for n in 2..16 {
        qps_bench(n, 10000)
    }
}

fn qps_bench(n: i32, round: i32) {
    let directory = Arc::new(RwLock::new(Directory::new(
        &"./data/db".to_string().add(n.to_string().as_str()),
    )));
    let barrier = Arc::new(Barrier::new(n as usize));

    let mut handles = Vec::with_capacity(n as usize);
    for i in 0..n {
        let b = barrier.clone();
        let idx = i.clone();
        let bl = directory.clone();

        let handle = thread::spawn(move || {
            let mut ct = CacheController::new(bl);
            let normal: Normal<f64> = Normal::new((idx * 200) as f64, 84 as f64).unwrap();

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
                if idx % 3 != 0 {
                    ct.set(key, (idx * 10 + i).to_string());
                } else {
                    ct.get(key);
                }
            }
            let end = start.elapsed();
            println!(
                "thread {:?} time cost: {:?} ms, QPS is {:?}",
                idx,
                end.as_millis(),
                ((round as f64) / end.as_secs_f64()) as i64,
            );
        });

        handles.push(handle);
    }
    let start = Instant::now();
    for handle in handles {
        handle.join().unwrap()
    }

    println!(
        "\n total average qps: {:?}",
        (((n * round) as f64) / (start.elapsed().as_secs_f64())) as i32
    )
}
