use mymesi::{BusLine, CacheController};
use rand;
use rand_distr::{Distribution, Normal};
use std::sync::{Arc, Mutex};

fn main() {
    let bus_line = Arc::new(Mutex::new(BusLine::new("./data/db")));

    let mut cache_controllers: Vec<CacheController<String>> = Vec::new();
    for _ in 0..2 {
        cache_controllers.push(CacheController::new(bus_line.clone(), "".to_string()))
    }

    cache_controllers[0].set("key2".to_string(), "val2".to_string());
    let val = cache_controllers[1].get("key2".to_string());
    println!("{:?}", val);

    let normal = Normal::new(0.0, 10.0).unwrap();
    for _ in 0..10 {
        let v = normal.sample(&mut rand::thread_rng());
        println!("{} is from a N(0, 3 distribution", v)
    }

    println!("finished");
}
