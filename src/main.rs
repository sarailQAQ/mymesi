use mymesi::{CacheController, Directory};
use parking_lot::RwLock;
use std::sync::{Arc};

fn main() {
    let directory = Arc::new(RwLock::new(Directory::new(&"./data/db".to_string())));

    let mut cache_controllers: Vec<CacheController<String>> = Vec::new();
    for _ in 0..2 {
        cache_controllers.push(CacheController::new(directory.clone()))
    }

    cache_controllers[0].set("key2".to_string(), "val2".to_string());
    let val = cache_controllers[1].get("key2".to_string());
    println!("{:?}", val);
}
