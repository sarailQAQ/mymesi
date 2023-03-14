use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::Instant;
use rand_distr::{Normal, Distribution};
use rand_distr::num_traits::ToPrimitive;
use mymesi::*;

#[test]
fn qps_test() {
    let n = 8;
    let round = 30000;

    let bus_line = Arc::new(Mutex::new(BusLine::new("./data/db")));
    let barrier = Arc::new(Barrier::new(n));

    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        let b = barrier.clone();
        let idx = i.clone();
        let bl = bus_line.clone();

        let handle = thread::spawn(move || {
            let mut ct =
                CacheController::new(bl, "".to_string());
            let  mut normal: Normal<f64> = Normal::new(0.0, 5.0).unwrap();

            b.wait();

            let start = Instant::now();
            for i in 0..round {

                let key = normal.sample(&mut rand::thread_rng()).abs().floor().to_i64().unwrap().to_string();
                if idx % 2 == 1 {
                    ct.set(key, (idx * 10 + i).to_string());
                } else {
                    let val = ct.get(key);
                }

            }
            println!("thread {:?} time cost: {:?} ms", idx, start.elapsed().as_millis());
        });

        handles.push(handle);
    }

    for handle in handles { handle.join().unwrap() }
}
