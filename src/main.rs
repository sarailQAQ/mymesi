use std::{sync::{Mutex, Arc}, thread};
use mymesi::{BusLine, CachesController};

fn main() {

    let bus_line  = Arc::new(Mutex::new(BusLine::new("./data/db")));

    let mut ct = CachesController::new(bus_line, "".to_string());

    println!("initial success");
    ct.set("key1".to_string(), "val1".to_string());
    println!("key1: {:?}", ct.get("key1".to_string()));

}
