mod db;
mod thread_socket;

use std::{
    sync::atomic::AtomicU8,
    sync::{mpsc, Arc, Mutex, RwLock},
    thread,
    fmt::Display
};
use futures::select;
use crate::db::db::DbSession;



pub struct Cache<T: Copy> {
    id: String,
    value: T,
    status: StatusFlag,
}

impl<T: Copy + ToString> Cache<T> {
    fn new(id: String, value: T) -> Cache<T> {
        let rwlock = RwLock::new(5);
        let status = STATUS_EXCLUSIVE;
        Cache {
            id,
            value,
            status,
        }
    }

    pub fn is(self, id: String) -> bool {
        id == self.id
    }

    pub fn get_value(self) -> T {
        return self.value.clone();
    }

    pub fn set_value(&mut self, val: T) {
        self.value = val;
    }

    pub fn get_status(self) -> StatusFlag {
        self.status.clone()
    }

    pub fn set_status(&mut self, status: StatusFlag) {
        let lock = RwLock::new()
        self.status = status;
    }
}

pub trait CacheHandler<T: Copy> {
    fn handle(&self, cache: &mut Cache<T>, event: EventType) ;
}

const CACHE_LENGTH: usize = 2 << 10;

pub type EventType = u8;
const EVENT_LOCAL_READ: EventType = 1;
const EVENT_LOCAL_WRITE: EventType = 2;
const EVENT_REMOTE_READ: EventType = 3;
const EVENT_REMOTE_WRITE: EventType = 4;

pub type StatusFlag = u8;
const STATUS_MODIFIED: StatusFlag = 1;
const STATUS_EXCLUSIVE: StatusFlag = 2;
const STATUS_SHARED: StatusFlag = 3;
const STATUS_INVALID: StatusFlag = 4;

struct ModifiedCacheHandler {
    db: &'static DbSession,
}

impl ModifiedCacheHandler {
    fn new(db: &'static DbSession) -> ModifiedCacheHandler {
        ModifiedCacheHandler {db}
    }
}

impl<T: Copy + ToString> CacheHandler<T> for ModifiedCacheHandler {
    fn handle(&self, cache: &mut Cache<T>, event: EventType) {
        match event {
            EVENT_LOCAL_READ => {},
            EVENT_LOCAL_WRITE => {},
            EVENT_REMOTE_READ => {
                self.db.set(cache.id.clone(), cache.value.clone());
                cache.set_status(STATUS_SHARED);
            },
            EVENT_REMOTE_WRITE => {
                cache.status = STATUS_INVALID;
                cache.set_status(STATUS_INVALID);
            },
            _ => {}
        };
    }
}

struct ExclusiveCacheHandler {
    db: &'static DbSession,
}

impl ExclusiveCacheHandler {
    fn new(db: &'static DbSession) -> ExclusiveCacheHandler {
        ExclusiveCacheHandler {db}
    }
}

impl<T: Copy + ToString> CacheHandler<T> for ExclusiveCacheHandler  {
    fn handle(&self, cache: &mut Cache<T>, event: EventType) {
        match event {
            EVENT_LOCAL_READ => {},
            EVENT_LOCAL_WRITE => {
                cache.set_status(STATUS_MODIFIED);
            },
            EVENT_REMOTE_READ => |cache: &mut Cache<T>| {
                cache.set_status(STATUS_SHARED);
            },
            EVENT_REMOTE_WRITE => |cache: &mut Cache<T>| {
                cache.set_status(STATUS_INVALID);
            },
            _ => {},
        };
    }
}

struct SharedCacheHandler {
    db: &'static DbSession,
}

impl SharedCacheHandler {
    fn new(db: &'static DbSession) -> SharedCacheHandler {
        SharedCacheHandler {db}
    }
}

impl<T: Copy + ToString> CacheHandler<T>  for SharedCacheHandler {
    fn handle(&self, cache: &mut Cache<T>, event: EventType) {
        match event {
            EVENT_LOCAL_READ => {},
            EVENT_LOCAL_WRITE => {
                cache.set_status(STATUS_MODIFIED);
            },
            EVENT_REMOTE_READ => {},
            EVENT_REMOTE_WRITE => {
                cache.set_status(STATUS_INVALID)
            },
            _ => {}
        }
    }
}

struct InvalidCacheHandler {
    db: &'static DbSession,
}

impl InvalidCacheHandler {
    fn new(db: &'static DbSession) -> InvalidCacheHandler {
        InvalidCacheHandler {db}
    }
}

impl<T: Copy + ToString> CacheHandler<T>  for SharedCacheHandler {
    fn handle(&self, cache: &mut Cache<T>, event: EventType) {
        match event {
            EVENT_LOCAL_READ => {

            },
            EVENT_LOCAL_WRITE => {
                cache.set_status(STATUS_MODIFIED);
            },
            EVENT_REMOTE_READ => {},
            EVENT_REMOTE_WRITE => {},
            _ => {}
        }
    }
}

struct Caches<T: Copy + ToString> {
    caches: [Cache<T>; 1024],
    bus: (),
}

struct BusLine {
    line_sender: mpsc::Sender<Event>,
    cache_receiver_template: Arc<Mutex<mpsc::Receiver<event>>>,

    cache_sender_template: Arc<Mutex<mpsc::Sender<Result<u8, u8>>>>,
    line_receiver: mpsc::Receiver<Result<u8, u8>>,

    threads: Vec<u8>,
}

impl BusLine {
    fn new() -> BusLine {
        let (line_sender, cache_receiver_template) = mpsc::channel();
        let cache_receiver_template = Arc::new(Mutex::new(cache_receiver_template));

        let (cache_sender_template, line_receiver) = mpsc::channel();
        let cache_sender_template = Arc::new(Mutex::new(cache_sender_template));

        let mut threads: Vec<u8> = Vec::new();

        let thread = thread::spawn(move || loop {

        });

        BusLine {
            line_sender, cache_receiver_template,
            cache_sender_template, line_receiver,
            threads }
    }

    fn register(&mut self) -> (u8, Arc<Mutex<mpsc::Receiver<event>>>, Arc<Mutex<mpsc::Sender<Result<u8, u8>>>>) {
        let id = self.threads.len() as u8;
        self.threads.push(id.clone());

        (id, self.cache_receiver_template.clone(), self.cache_sender_template.clone())
    }
}

struct Event {
    id: String,
    event_type: EventType,
    thread_id: u8,
}

struct Request {

}

type Response = Result<u8, u8>;

enum Something {
    Event,

    Response,
}