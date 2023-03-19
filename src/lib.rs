pub mod db;
pub mod thread_socket;

use crate::db::db::DbSession;
use crate::thread_socket::thread_socket::{new_socket, ThreadSocket};
use std::collections::VecDeque;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
};
use async_std::io::WriteExt;
use dashmap::DashMap;
use futures::SinkExt;


pub struct Cache<T: Clone + ToString + Sync> {
    id: String,
    value: T,
    status: Status,
}

impl<T: Clone + ToString + Sync> Cache<T> {
    fn new(id: String, value: T, status: Status) -> Cache<T> {
        Cache { id, value, status }
    }

    pub fn is(self, id: &String) -> bool {
        *id == self.id
    }

    pub fn get(self) -> T {
        return self.value.clone();
    }

    pub fn set(&mut self, val: T) {
        self.value = val;
    }

    /// `handle` 如果缓存的状态为 Invalid，返回 true
    fn handle(&mut self, event: &Event) -> bool {
        if self.status == Status::Invalid {
            return true;
        }
        match event {
            Event::RemoteRead(_) => {
                self.status = Status::Shared;
                false
            }
            Event::RemoteWrite(_) => {
                self.status = Status::Invalid;
                true
            }
        }
    }

    fn flush(&self, session: DbSession) {
        if self.status != Status::Modified {
            return;
        }

        session.set(self.id.clone(), self.value.to_string());
    }
}

unsafe impl<T: Clone + ToString + Sync> Send for Cache<T> {}

#[derive(Debug)]
enum Event {
    RemoteRead(String),
    RemoteWrite(String),
}

impl Event {
    fn get_id(&self) -> &String {
        match self {
            Event::RemoteRead(id) => id,
            Event::RemoteWrite(id) => id,
        }
    }
}

impl Clone for Event {
    fn clone(&self) -> Self {
        match self {
            Event::RemoteRead(id) => Event::RemoteRead(id.clone()),
            Event::RemoteWrite(id) => Event::RemoteWrite(id.clone()),
        }
    }
}

#[derive(Debug, PartialEq)]
enum Status {
    Modified,
    Exclusive,
    Shared,
    Invalid,
}

#[derive(Debug, Clone)]
enum Message {
    EventMsg(Event),

    // 是否持有对应数据的缓存
    Response(bool),
}

type ThreadID = usize;

const CACHE_SIZE: usize = 1 << 10;
const FLUSH_SIZE: usize = 1 << 6;

#[derive(Clone)]
pub struct CacheController<T: Clone + ToString + Sync> {
    caches: Arc<Mutex<HashMap<String, Cache<T>>>>,
    invalid_queue: Arc<Mutex<VecDeque<String>>>,
    bus_line: Arc<Mutex<BusLine>>,
    db: DbSession,
    thread_id: ThreadID,

    // 一些测试指标
    op_cnt: u32,       // 总操作次数
    in_cache_cnt: u32, // 缓存命中次数
}

impl<T: Clone + ToString + Sync + From<String> + 'static> CacheController<T> {
    pub fn new(bus_line: Arc<Mutex<BusLine>>, _val: T) -> CacheController<T> {
        let caches: Arc<Mutex<HashMap<String, Cache<T>>>> =
            Arc::new(Mutex::new(HashMap::with_capacity(CACHE_SIZE)));

        let mut line = bus_line.lock().unwrap();
        let (thread_id, socket) = line.register();
        let db = line.db.clone();
        drop(line);

        let invalid_queue: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

        // 启动一个线程，监听来自 bus_line 的消息
        let t_id = thread_id.clone();
        let mut _caches = caches.clone();
        let _db = db.clone();
        let _q = invalid_queue.clone();
        thread::spawn(move || {
            loop {
                let msg = socket.receive();
                // println!(
                //     "thread {:?} receive message: {:?}",
                //     t_id.clone(),
                //     msg.clone()
                // );

                let event = match msg {
                    Message::EventMsg(e) => e,
                    Message::Response(_) => {
                        panic!("thread {:?} receives an invalid message", t_id.clone())
                    }
                };
                let id = event.get_id();

                // 收到消息 bus_line 一定处于 lock 状态
                let mut caches = _caches.lock().unwrap();
                socket.send(match caches.get_mut(id) {
                    Some(cache) => {
                        if cache.status == Status::Modified {
                            cache.flush(_db.clone())
                        }
                        let is_invalid = cache.handle(&event);
                        if is_invalid {
                            caches.remove(id);
                        }

                        Message::Response(!is_invalid)
                    }
                    None => Message::Response(false),
                });

                if caches.len() < CACHE_SIZE {
                    continue;
                }

                let mut q = _q.lock().unwrap();
                for _ in 0..FLUSH_SIZE {
                    while !q.is_empty() && !caches.contains_key(q.front().unwrap()) {
                        q.pop_front();
                    }

                    caches.remove(q.front().unwrap());
                }
            }
        });

        CacheController {
            caches,
            invalid_queue,
            bus_line,
            db,
            thread_id,
            op_cnt: 0,
            in_cache_cnt: 0,
        }
    }

    pub fn get(&mut self, id: String) -> T {
        // 预处理
        self.op_cnt += 1;
        let message = Message::EventMsg(Event::RemoteRead(id.clone()));

        {
            let caches = self.caches.lock().unwrap();
            let cache = caches.get(&*id);
            if matches!(cache, Some(c) if c.status != Status::Invalid) {
                self.in_cache_cnt += 1;
                return cache.unwrap().value.clone();
            }
        }

        let bus_line = self.bus_line.lock().unwrap();
        let mut caches = self.caches.lock().unwrap();

        let n = bus_line.broadcast(self.thread_id.clone(), message);
        drop(bus_line);

        let val = T::from(self.db.get(id.clone()));
        let status = if n > 0 {
            Status::Shared
        } else {
            Status::Exclusive
        };
        let c = Cache::new(id.clone(), val.clone(), status);
        caches.insert(id.clone(), c);
        self.invalid_queue.lock().unwrap().push_back(id);
        val
    }

    pub fn set(&mut self, id: String, val: T) {
        // 预处理
        self.op_cnt += 1;
        let message = Message::EventMsg(Event::RemoteWrite(id.clone()));

        let bus_line = self.bus_line.lock().unwrap();
        let mut caches = self.caches.lock().unwrap();

        match caches.get_mut(&*id) {
            Some(c) => {
                if c.status == Status::Shared {
                    bus_line.broadcast(self.thread_id.clone(), message);
                    drop(bus_line)
                }
                self.in_cache_cnt += 1;
                c.value = val;
                c.status· = Status::Modified;
            }
            None => {
                bus_line.broadcast(self.thread_id.clone(), message);
                drop(bus_line);
                let status = Status::Modified;
                let c = Cache::new(id.clone(), val.clone(), status);
                caches.insert(id.clone(), c);
                self.invalid_queue.lock().unwrap().push_back(id);
            }
        };
    }
}

impl<T: Clone + Sync + ToString> Drop for CacheController<T> {
    fn drop(&mut self) {
        println!(
            "线程 {:?} 总操作次数：{:?}，缓存命中次数：{:?}",
            self.thread_id, self.op_cnt, self.in_cache_cnt
        );
    }
}

pub struct BusLine {
    sockets: Vec<ThreadSocket<Message>>,
    db: DbSession,
}

impl BusLine {
    pub fn new(db_path: &'static str) -> BusLine {
        let sockets: Vec<ThreadSocket<Message>> = Vec::new();
        let db = DbSession::new(db_path);

        BusLine { sockets, db }
    }

    fn register(&mut self) -> (ThreadID, ThreadSocket<Message>) {
        let (s1, s2) = new_socket();
        self.sockets.push(s1);

        let id = (self.sockets.len() - 1) as ThreadID;

        (id, s2)
    }

    fn broadcast(&self, thread_id: ThreadID, message: Message) -> u8 {
        match message {
            Message::EventMsg(_) => {}
            Message::Response(_) => panic!("invalid msg from {:?}", thread_id),
        };

        // println!("bus_line will broadcast {:?}", message.clone());

        let mut handle_count = 0;
        for i in 0..self.sockets.len() {
            if i as ThreadID == thread_id {
                continue;
            }
            self.sockets[i].send(message.clone());
        }

        for i in 0..self.sockets.len() {
            if i as ThreadID == thread_id {
                continue;
            }

            let msg = self.sockets[i].receive();
            handle_count += matches!(msg, Message::Response(f) if f) as u8;
        }

        handle_count
    }
}

/// `Directory` 缓存目录
/// 使用实现了 shard 特性的 DashMap 提高系统并发度
struct Directory {
    map: DashMap<String, VecDeque<ThreadID>>, // shard 数量为 cpu核数/线程数 * 4 * 2
    sockets: Vec<ThreadSocket<Message>>,
    db: DbSession,
}

impl Directory {
    fn new(db_path: &'static str) -> Directory {
        let map = DashMap::new();
        let sockets: Vec<ThreadSocket<Message>> = Vec::new();
        let db = DbSession::new(db_path);

        Directory { map, sockets, db}
    }

    fn register(&mut self) -> (ThreadID, ThreadSocket<Message>) {
        let (s1, s2) = new_socket();
        self.sockets.push(s1);

        let id = (self.sockets.len() - 1) as ThreadID;

        (id, s2)
    }

    fn broadcast(&self, thread_id: ThreadID, event: Event, ids: &VecDeque<ThreadID>) -> usize {
        let n = ids.len();

        for i in ids {
            if *i == thread_id { continue; }
            self.sockets[*i].send(msg.clone());
        }

        n
    }

    // 从 db 读取数据
    fn read<T: Clone + Sync + From<String>>(&self, thread_id: ThreadID,id: String) -> (T, usize) {
        let threads = self.map.get_mut(&*id.clone());
        let mut n = 0;
        match threads {
            None => self.map.insert(id.clone(), VecDeque::from(vec![thread_id])),
            Some(mut v) => {
                self.broadcast(thread_id, Event::RemoteRead(id.clone()), v.value());
                n = v.len();
                v.value().push_back(thread_id.clone());
            }
        };


        (self.db.get(id).into(), n)
    }

    // 将数据写回 db，
    fn write_back<T: Clone + Sync + ToString>(&self, id: String, val: T) {
        self.db.set(id, val.to_string());
    }

    // 维护目录，并广播msg
    fn write_to_cache<T: Clone + Sync + From<String>>(&self, thread_id: ThreadID, id: String, need_read: bool) -> Option<T> {
        // 广播 message

        // 更新目录
        let threads = self.map.get_mut(&*id.clone());
        match threads {
            None => self.map.insert(id.clone(), VecDeque::from(vec![thread_id])),
            Some(mut v) => {
                let mut v = v.value_mut();
                self.broadcast(thread_id, Event::RemoteWrite(id.clone()), v);

                while !v.is_empty() && v.front().unwrap() != thread_id { v.pop_front(); }
                while !v.is_empty() && v.back().unwrap() != thread_id { v.pop_back(); }
            }
        };

        if need_read { Some(self.db.get(id)).into() } else { None }
    }

    fn remove(&self, thread_id: ThreadID, ids: Vec<String>) {
        for id in ids {
            let threads = self.map.get_mut(&*id.clone());

            match threads {
                None => {},
                Some(mut v) => {
                    let mut v = v.value_mut();
                    let mut idx = -1;
                    for i in 0..v.len() {
                        if t_id == thread_id {
                            idx = i;
                            break;
                        }
                    }
                    v.remove(idx);
                }
            }
        }
    }
}
