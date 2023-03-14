use mymesi::*;
use std::{
    sync::{Arc, Mutex, Barrier},
    thread,
    time,
};
use rand_distr::Normal;

/// `concurrency_safety_test`
/// 并发安全测试，确保并发操作时的互斥操作串行执行
/// 通过查看 log 确认
#[test]
fn concurrency_safety_test() {
    let n = 4;
    let round = 3;

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

            b.wait();

            for i in 0..round {
               if idx % 2 == 1 {
                   println!("tread {:?} set the key", idx.clone());
                   ct.set("key".to_string(), (idx * 10 + i).to_string());
               } else {
                   let val = ct.get("key".to_string());
                   println!("tread {:?} get the key, value: {:?}", idx.clone(), val);
               }
                thread::sleep(time::Duration::from_millis(10));
            }
        });

        handles.push(handle);
    }

    for handle in handles { handle.join().unwrap() }
}