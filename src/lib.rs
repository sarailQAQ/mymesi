mod db;
mod thread_socket;

use std::{
    sync::{Arc, Mutex},
    thread,
    collections::{
        HashMap,
    },
};
use crate::db::db::DbSession;
use crate::thread_socket::thread_socket::{new_socket, ThreadSocket};


pub struct Cache<T: Clone + ToString + Sync> {
    id: String,
    value: T,
    status: StatusFlag,
}

impl<T: Clone + ToString + Sync> Cache<T> {
    fn new(id: String, value: T) -> Cache<T> {
        let status = STATUS_INVALID;
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
        self.status = status;
    }
}

unsafe impl<T: Clone + ToString + Sync> Send for Cache<T>{}

fn new_cache_handlers<T: Clone + ToString + Sync + From<String>>(db: Arc<Mutex<DbSession>>)
    -> HashMap<StatusFlag, Box<dyn CacheHandler<T>>>
{

    let mut map: HashMap<StatusFlag, Box<dyn CacheHandler<T>>> = HashMap::new();

    map.insert(STATUS_MODIFIED, Box::new(ModifiedCacheHandler::new(db.clone())));
    map.insert(STATUS_EXCLUSIVE, Box::new(ExclusiveCacheHandler::new(db.clone())));
    map.insert(STATUS_SHARED, Box::new(SharedCacheHandler::new(db.clone())));
    map.insert(STATUS_INVALID, Box::new(InvalidCacheHandler::new(db.clone())));

    map
}

pub trait CacheHandler<T: Clone + ToString + Sync> {
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
    db: Arc<Mutex<DbSession>>,
}

impl  ModifiedCacheHandler {
    fn new(db: Arc<Mutex<DbSession>>) -> ModifiedCacheHandler {
        ModifiedCacheHandler { db }
    }
}

impl<T: Clone + ToString + Sync> CacheHandler<T> for ModifiedCacheHandler {
    fn handle(&self, cache: &mut Cache<T>, event: EventType) {
        match event {
            EVENT_LOCAL_READ => {},
            EVENT_LOCAL_WRITE => {},
            EVENT_REMOTE_READ => {
                let session = self.db.lock().unwrap();
                session.set(cache.id.clone(), cache.value.to_string());
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
    db: Arc<Mutex<DbSession>>
}

impl ExclusiveCacheHandler {
    fn new(db: Arc<Mutex<DbSession>>) -> ExclusiveCacheHandler {
        ExclusiveCacheHandler { db }
    }
}

impl<T: Clone + ToString + Sync> CacheHandler<T> for ExclusiveCacheHandler  {
    fn handle(&self, cache: &mut Cache<T>, event: EventType) {
        match event {
            EVENT_LOCAL_READ => {},
            EVENT_LOCAL_WRITE => {
                cache.set_status(STATUS_MODIFIED);
            },
            EVENT_REMOTE_READ => {
                cache.set_status(STATUS_SHARED);
            },
            EVENT_REMOTE_WRITE =>  {
                cache.set_status(STATUS_INVALID);
            },
            _ => {},
        };
    }
}

struct SharedCacheHandler {
    db: Arc<Mutex<DbSession>>
}

impl SharedCacheHandler {
    fn new(db: Arc<Mutex<DbSession>>) -> SharedCacheHandler {
        SharedCacheHandler { db }
    }
}

impl<T: Clone + ToString + Sync> CacheHandler<T>  for SharedCacheHandler {
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
    db: Arc<Mutex<DbSession>>
}

impl InvalidCacheHandler {
    fn new(db: Arc<Mutex<DbSession>>) -> InvalidCacheHandler {
        InvalidCacheHandler { db }
    }
}

impl<T: Clone + ToString + Sync + From<String>> CacheHandler<T>  for InvalidCacheHandler {
    fn handle(&self, cache: &mut Cache<T>, event: EventType) {
        // 在 handle 前，需要先广播，等其他所有节点响应后再进行下一步动作
        match event {
            EVENT_LOCAL_READ => {
                let session = self.db.lock().unwrap();
                let val = session.get(cache.id.clone());
                cache.value = val.into();
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

pub struct CacheController<T: Clone + ToString + Sync> {
    caches: Arc<Mutex<Vec<Cache<T>>>>,
    bus_line: Arc<Mutex<BusLine>>,
    thread_id: u8,
    // socket: ThreadSocket<Message>,
    tail: usize,
}

impl<T: Clone + ToString + Sync + From<String> + 'static> CacheController<T> {
    pub fn new(bus_line: Arc<Mutex<BusLine>>, _val: T) -> CacheController<T> {
        let caches: Arc<Mutex<Vec<Cache<T>>>> =
            Arc::new(Mutex::new(Vec::with_capacity(1024)));

        let mut line = bus_line.lock().unwrap();
        let (thread_id, socket) = line.register();
        let db = line.db.clone();
        drop(line);

        // 启动一个线程，监听来自 bus_line 的消息
        let t_id = thread_id.clone();
        let mut _caches = caches.clone();
        thread::spawn(move || {
            let handlers = new_cache_handlers(db);
            loop {
                let msg = socket.receive();
                println!("thread {:?} receive message: {:?}", t_id.clone(), msg.clone());

                let mut resp = Message {
                    id: msg.id.clone(),
                    event_type: 0,
                    status: 0,
                    thread_id: t_id.clone(),
                };

                //  get / set 函数会取得 caches 的锁
                // 如果不跳过会导致死锁
                if msg.thread_id == t_id {
                    socket.send(resp);
                    continue
                }

                // 收到消息 bus_line 一定处于 lock 状态
                let mut caches = _caches.lock().unwrap();
                let mut index = None;
                for i in 0..caches.len() {
                    if caches[i].id == msg.id {
                        index = Some(i.clone());
                    }
                }

                match index {
                    None => {},
                    Some(i) => {
                        let handler = handlers.get(&caches[i].status).unwrap();
                        handler.handle(&mut caches[i], msg.event_type);
                        resp.status = caches[i].status.clone();
                    }
                }

                socket.send(resp);
            }
        });


        let tail = 0 as usize;
        CacheController { caches, bus_line, thread_id, tail }
    }

    pub fn get(&mut self, id: String) -> T {
        // 预处理
        let event_type = EVENT_LOCAL_READ;
        let mut index = None;

        let bus_line = self.bus_line.lock().unwrap();
        let mut caches = self.caches.lock().unwrap();

        for i in 0..caches.len() {
            if id == caches[i].id  {
                index = Some(i);
            }
        }

        let status = match index {
            None => STATUS_INVALID,
            Some(i) => {
                caches[i].status.clone()
            },
        };

        let message = Message {
            id: id.clone(),
            event_type,
            status,
            thread_id: self.thread_id.clone(),
        };

        let n = bus_line.broadcast(message);


        match index {
            None => {
                let val: T = bus_line.load(id.clone());
                let mut c = Cache::new(id, val.clone());
                c.status = if n == 0 { STATUS_EXCLUSIVE } else { STATUS_SHARED };

                caches.push(c);
                val
            }
            Some(i) => caches[i].value.clone(),
        }
    }

    pub fn set(&mut self, id: String, val: T) {
        // 预处理
        let event_type = EVENT_LOCAL_WRITE;
        let mut index = None;

        let bus_line = self.bus_line.lock().unwrap();
        let mut caches = self.caches.lock().unwrap();

        for i in 0..caches.len() {
            if id == caches[i].id  {
                index = Some(i);
            }
        }

        let status = match index {
            None => STATUS_INVALID,
            Some(i) => caches[i].status.clone(),
        };

        let message = Message {
            id: id.clone(),
            event_type,
            status,
            thread_id: self.thread_id.clone(),
        };

        let n = bus_line.broadcast(message);

        match index {
            None => {
                bus_line.write_back(id.clone(), val.clone());
                let mut c = Cache::new(id, val);
                c.status = if n == 0 { STATUS_EXCLUSIVE } else { STATUS_SHARED };

                caches.push(c)
            }
            Some(i) => caches[i].value = val,
        };
    }
}

pub struct BusLine {
    sockets: Vec<ThreadSocket<Message>>,
    db: Arc<Mutex<DbSession>>,
}

impl BusLine {
    pub fn new(db_path: &'static str) -> BusLine {
        let sockets: Vec<ThreadSocket<Message>> = Vec::new();
        let db = Arc::new(Mutex::new(DbSession::new(db_path)));
        BusLine { sockets, db }
    }

    fn register(&mut self) -> (u8, ThreadSocket<Message>) {
        let (s1, s2) = new_socket();
        self.sockets.push(s1);

        let id = (self.sockets.len() - 1)  as u8;

        (id, s2)
    }

    fn broadcast(&self, event_info: Message) -> u8{
        let event_type = match event_info.event_type  {
            EVENT_LOCAL_READ => EVENT_REMOTE_READ,
            EVENT_LOCAL_WRITE => EVENT_REMOTE_WRITE,
            _ => event_info.event_type,
        };

        let message = Message {
            id: event_info.id,
            event_type,
            status: event_info.status,
            thread_id: event_info.thread_id,
        };
        println!("bus_line will broadcast {:?}", message.clone());

        let mut handle_count = 0;
        for i in 0..self.sockets.len()  {
            if i as u8 == event_info.thread_id { continue }
            self.sockets[i].send(message.clone());

            let msg = self.sockets[i].receive();

            if msg.status != STATUS_INVALID { handle_count = handle_count + 1 };
        }

        handle_count
    }

    fn load<T: From<String> >(&self, id: String) -> T {
        self.db.lock().unwrap().get(id).into()
    }

    fn write_back<T: ToString>(&self, id: String, val: T) {
        self.db.lock().unwrap().set(id, val.to_string());
    }
}

#[derive(Debug)]
struct Message {
    id: String,
    event_type: EventType,
    status: StatusFlag,
    thread_id: u8,
}

impl Clone for Message {
    fn clone(&self) -> Self {
        Message {
            // id: String::from(self.id.clone()),
            id: self.id.clone(),
            event_type: self.event_type.clone(),
            status: self.status.clone(),
            thread_id: self.thread_id.clone(),
        }
    }
}



