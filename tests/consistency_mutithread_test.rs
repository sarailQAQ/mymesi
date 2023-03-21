use mymesi::*;
use parking_lot::RwLock;
use rand::Rng;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Distribution, Normal};
use std::ops::Add;
use std::sync::{Arc, Barrier, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn consistency_multithread_test() {
    for i in 0..10 {
        sub_consistency_multithread_test(i.clone());
    }
}

fn sub_consistency_multithread_test(id: usize) {
    let n = 4 as i32;
    let round = 5000 as i32;

    let directory = Arc::new(RwLock::new(Directory::new(
        &"./data/db".to_string().add(id.to_string().as_str()),
    )));
    let barrier = Arc::new(Barrier::new(n as usize));

    let collect = Arc::new(Mutex::new(Vec::new()));

    let mut handles = Vec::with_capacity(n as usize);
    for i in 0..n {
        let collect = collect.clone();
        let b = barrier.clone();
        let idx = i.clone();
        let bl = directory.clone();

        let handle = thread::spawn(move || {
            let mut ct = CacheController::new(bl);

            b.wait();

            let mut rng = rand::thread_rng();
            let start = Instant::now();
            for i in 0..round {
                let key = rng.gen_range(0..1024).to_string();
                if idx % 3 != 0 {
                    ct.set(key, (idx * 10 + i).to_string());
                } else {
                    ct.get(key);
                }
            }
            let end = start.elapsed();

            collect.lock().unwrap().push(ct.collect_caches());
        });

        handles.push(handle);
    }

    thread::sleep(Duration::from_secs(1));

    for handle in handles {
        handle.join().unwrap()
    }

    for i in 0..1024 {
        let mut caches: Vec<Cache<String>> = Vec::new();
        let caches_set = collect.lock().unwrap();
        for caches_map in caches_set.to_vec() {
            match caches_map.get(&*i.to_string()) {
                None => {}
                Some(c) => {
                    caches.push(Cache::new(
                        c.key().clone(),
                        c.value.clone(),
                        c.status.clone(),
                    ));
                }
            }
        }

        if caches.len() == 0 {
            continue;
        } else if caches.len() == 1 {
            if caches[0].status == Status::Shared {
                panic!("exclusive cache has the status shared");
            }
        } else {
            for c in caches {
                if c.status == Status::Modified || c.status == Status::Exclusive {
                    panic!("shared caches has status of {:?}", c.status.clone());
                }
            }
        }
    }
    println!("test {:?} past", id.clone());
}
