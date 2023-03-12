use std::{
    sync::{Mutex, Arc}
};
use mymesi::{
    BusLine
};

fn main() {
    let bus_line = BusLine::new("./data/db");
    let counter = Arc::new(Mutex::new(bus_line));
}
