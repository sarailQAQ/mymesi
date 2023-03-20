use mymesi::{CacheController, Directory};
use rand;
use rand_distr::{Distribution, Normal};
use std::sync::{Arc, Barrier, Mutex};
use std::{thread, time};
use parking_lot::RwLock;

fn main() {
    // let directory = Arc::new(RwLock::new(Directory::new("./data/db")));
    //
    // let mut cache_controllers: Vec<CacheController<String>> = Vec::new();
    // for _ in 0..2 {
    //     cache_controllers.push(CacheController::new(directory.clone()))
    // }
    //
    // cache_controllers[0].set("key2".to_string(), "val2".to_string());
    // let val = cache_controllers[1].get("key2".to_string());
    // println!("{:?}", val);
    let n = 4;
    let round = 10000;

    let directory = Arc::new(RwLock::new(Directory::new("./data/db")));
    let barrier = Arc::new(Barrier::new(n));

    let mut handles = Vec::with_capacity(n);
    for i in 0..n {
        let b = barrier.clone();
        let idx = i.clone();
        let bl = directory.clone();

        let handle = thread::spawn(move || {
            let mut ct = CacheController::new(bl);

            b.wait();

            for i in 0..round {
                if i % 2 == 1 {
                    println!("tread {:?} set the key", ct.thread_id.clone());
                    ct.set("key".to_string(), (idx * 10 + i).to_string());
                } else {
                    println!("tread {:?} access the key", ct.thread_id.clone());
                    let val = ct.get("key".to_string());
                    // println!("tread {:?} get the key, value: {:?}", ct.thread_id.clone(), val);
                }
                thread::sleep(time::Duration::from_millis(1));
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap()
    }

}
