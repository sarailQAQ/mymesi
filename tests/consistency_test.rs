use mymesi::*;
use rand::Rng;
use rand_distr::num_traits::ToPrimitive;
use rand_distr::{Distribution, Normal};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// `consistency_test` 一致性测试
/// 用于测试缓存一致性
#[test]
fn consistency_test() {
    let mut map: HashMap<String, String> = HashMap::new();
    let mut read_count: HashMap<String, i32> = HashMap::new();
    let mut write_count: HashMap<String, i32> = HashMap::new();

    let n = 4; // 线程数
    let round = 100000; // 测试次数

    let bus_line = Arc::new(Mutex::new(BusLine::new("./data/db")));

    let mut cache_controllers: Vec<CacheController<String>> = Vec::new();
    for _ in 0..n {
        cache_controllers.push(CacheController::new(bus_line.clone(), "".to_string()));
    }

    let start = Instant::now();
    let mut rng = rand::thread_rng();
    for i in 0..round {
        let op: i32 = rng.gen();
        let t_id = rng.gen_range(0..n);
        let key = key_gen();

        if op % 4 != 0 {
            let val = cache_controllers[t_id].get(key.clone());
            let cnt = read_count.entry(key.clone()).or_insert(0);
            *cnt += 1;

            assert_eq!(
                match map.get(&key.clone()) {
                    None => "".to_string(),
                    Some(s) => s.to_string(),
                },
                val
            )
        } else {
            cache_controllers[t_id].set(key.clone(), i.to_string());
            map.insert(key.clone(), i.to_string());

            let cnt = write_count.entry(key).or_insert(0);
            *cnt += 1;
        }
    }

    for (k, v) in write_count {
        println!("key {:?} has benn writen {:?} times", k, v);
    }
    println!("time cost: {:?} ms", start.elapsed().as_millis());
    println!("consistency test passed");
}

/// `key_gen` 生成正态分布的随机数
fn key_gen() -> String {
    let normal: Normal<f64> = Normal::new(0.0, 5.0).unwrap();
    normal
        .sample(&mut rand::thread_rng())
        .abs()
        .floor()
        .to_i64()
        .unwrap()
        .to_string()
}
