pub mod db;
pub mod thread_socket;

use crate::db::db::DbSession;
use crate::thread_socket::thread_socket::{new_socket, ThreadSocket};
use dashmap::DashMap;
use std::collections::VecDeque;
use std::{
    sync::{Arc, Mutex},
    thread,
};
use parking_lot::RwLock;

#[derive(Debug)]
pub enum Event {
    RemoteRead(String),
    RemoteWrite(String),
    Confirmed,
}

impl Event {
    fn get_id(&self) -> &String {
        match self {
            Event::RemoteRead(id) => id,
            Event::RemoteWrite(id) => id,
            Event::Confirmed => panic!("Confirmed event has no id"),
        }
    }
}

impl Clone for Event {
    fn clone(&self) -> Self {
        match self {
            Event::RemoteRead(id) => Event::RemoteRead(id.clone()),
            Event::RemoteWrite(id) => Event::RemoteWrite(id.clone()),
            Event::Confirmed => Event::Confirmed,
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

type ThreadID = usize;

const CACHE_SIZE: usize = 1 << 10;
const FLUSH_SIZE: usize = 1 << 6;

#[derive(Clone)]
pub struct CacheController<T: Clone + ToString + Sync> {
    caches: Arc<RwLock<VecDeque<Cache<T>>>>,
    directory: Arc<RwLock<Directory>>,
    pub thread_id: ThreadID,

    // 一些测试指标
    op_cnt: u32, // 总操作次数
    in_cache_cnt: u32, // 缓存命中次数
}

impl<T: Clone + ToString + Sync + From<String> + 'static> CacheController<T> {
    pub fn new(directory: Arc<RwLock<Directory>>) -> CacheController<T> {
        let caches: Arc<RwLock<VecDeque<Cache<T>>>> = Arc::new(RwLock::new(VecDeque::new()));

        let mut directory = directory.clone();
        let (thread_id, socket) = directory.write().register();

        // 启动一个线程，监听来自 bus_line 的消息
        let t_id = thread_id.clone();
        let mut _caches = caches.clone();
        let _q = invalid_queue.clone();
        let _directory = directory.clone();
        thread::spawn(move || {
            loop {
                let event = socket.receive();
                // println!(
                //     "thread {:?} receive message: {:?}",
                //     t_id.clone(),
                //     event.clone()
                // );

                let id = event.get_id();

                let mut caches = _caches.write();
                let mut idx = None;
                for i in 0..caches.len() {
                    if caches[i].is(id) {
                        idx = Some(i);
                    }
                }

                match idx {
                    None => {}
                    Some(i) => {
                        if caches[i].status == Status::Modified {
                            _directory.read().write_back(cache.id.clone(), cache.value.clone());
                        }
                        if caches[i].handle(&event) {
                            caches.remove(i);
                        }
                    }
                };

                socket.send(Event::Confirmed);
                // if _caches.len() < CACHE_SIZE {
                //     continue;
                // }

                // let mut q = _q.lock().unwrap();
                // for _ in 0..FLUSH_SIZE {
                //     while !q.is_empty() && !_caches.contains_key(q.front().unwrap()) {
                //         q.pop_front();
                //     }
                //
                //     _caches.remove(q.front().unwrap());
                // }
            }
        });

        CacheController {
            caches,
            directory,
            thread_id,
            op_cnt: 0,
            in_cache_cnt: 0,
        }
    }

    // 会使用 caches 的读锁
    fn find_index(&self, id: String) -> Option<usize>{
        let mut caches = _caches.read();
        let mut idx = None;
        for i in 0..caches.len() {
            if caches[i].is(&id) {
                idx = Some(i);
            }
        }
        idx
    }

    pub fn get(&mut self, id: String) -> T {
        // 预处理
        self.op_cnt += 1;

        {
            // 命中缓存
            let caches = self.caches.read();
            for i in 0..caches.len() {
                if caches[i].is(&id) {
                    self.in_cache_cnt += 1;
                    return caches[i].value.clone();
                }
            }
            // 没有命中，先释放读锁
        }

        let (v, n): (T, usize) = self.directory.read().read(self.thread_id.clone(), id.clone());
        let status = if n > 0 {
            Status::Shared
        } else {
            Status::Exclusive
        };
        self.caches.write().push_back(Cache::new(id, v.clone(), status));
        v
    }

    pub fn set(&mut self, id: String, val: T) {
        // 预处理
        self.op_cnt += 1;

        self.directory.read()
            .write_to_cache(self.thread_id.clone(), id.clone());
        // 命中缓存
        let mut caches = self.caches.write();
        for i in 0..caches.len() {
            if caches[i].is(&id) {
                if caches[i].status != Status::Shared {
                    caches[i].value = val;
                    return;
                }
            }
        }

        let c = Cache::new(id.clone(), val.clone(), Status::Modified);
        caches.push_back(c);
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

pub struct Cache<T: Clone + ToString + Sync> {
    id: String,
    value: T,
    status: Status,
}

impl<T: Clone + ToString + Sync> Cache<T> {
    fn new(id: String, value: T, status: Status) -> Cache<T> {
        Cache { id, value, status }
    }

    pub fn is(&self, id: &String) -> bool {
        *id == self.id
    }

    pub fn get(&self) -> T {
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
            Event::Confirmed => panic!("Cache can not handle confirmed event"),
        }
    }
}

unsafe impl<T: Clone + ToString + Sync> Send for Cache<T> {}

/// `Directory` 缓存目录
/// 使用实现了 shard 特性的 DashMap 提高系统并发度
pub struct Directory {
    map: DashMap<String, VecDeque<ThreadID>>,
    // shard (数量为 cpu核数/线程数 * 4).next_power_of_2
    sockets: Mutex<Vec<ThreadSocket<Event>>>,
    db: DbSession,
}

impl Directory {
    pub fn new(db_path: &'static str) -> Directory {
        let map = DashMap::new();
        let sockets = Mutex::new(Vec::new());
        let db = DbSession::new(db_path);

        Directory { map, sockets, db }
    }

    pub fn register(&mut self) -> (ThreadID, ThreadSocket<Event>) {
        let (s1, s2) = new_socket();
        self.sockets.push(s1);

        let id = (self.sockets.len() - 1) as ThreadID;

        (id, s2)
    }

    fn broadcast(&self, thread_id: ThreadID, event: Event, ids: &VecDeque<ThreadID>) {
        if ids.len() == 0 {
            return;
        }

        // println!("directory will broadcast event: {:?} from {:?}", event, thread_id);

        let sockets = self.sockets.lock().unwrap();
        for i in ids {
            if *i == thread_id { continue; }
            sockets[*i].send(event.clone());
        }
        for i in ids {
            if *i == thread_id { continue; }
            sockets[*i].receive();
        }
    }

    // 从 db 读取数据
    fn read<T: Clone + Sync + From<String>>(&self, thread_id: ThreadID, id: String) -> (T, usize) {
        match self.map.get_mut(&*id.clone()) {
            None => {}
            Some(mut v) => {
                self.broadcast(thread_id, Event::RemoteRead(id.clone()), v.value());
                v.value_mut().push_back(thread_id.clone());
                return (self.db.get(id).into(), v.len());
            }
        };

        // 没有其他线程持有缓存，不需要广播
        self.map.insert(id.clone(), VecDeque::from(vec![thread_id]));
        (self.db.get(id).into(), 0)
    }

    // 将数据写回 db，
    fn write_back<T: Clone + Sync + ToString>(&self, id: String, val: T) {
        self.db.set(id, val.to_string());
    }

    // 维护目录，并广播msg
    fn write_to_cache(&self, thread_id: ThreadID, id: String) {
        // 更新目录
        match self.map.get_mut(&*id.clone()) {
            None => {}
            Some(mut v) => {
                let v = v.value_mut();
                // 广播 message
                self.broadcast(thread_id, Event::RemoteWrite(id.clone()), v);

                while !v.is_empty() && *(v.front().unwrap()) != thread_id {
                    v.pop_front();
                }
                while !v.is_empty() && *(v.back().unwrap()) != thread_id {
                    v.pop_back();
                }
                v.push_back(thread_id);
                return;
            }
        };

        self.map.insert(id.clone(), VecDeque::from(vec![thread_id]));
    }

    async fn remove(&self, thread_id: ThreadID, ids: Vec<String>) {
        for id in ids {
            let threads = self.map.get_mut(&*id.clone());

            match threads {
                None => {}
                Some(mut v) => {
                    let v = v.value_mut();
                    let mut idx = v.len();
                    for i in 0..v.len() {
                        if v[i] == thread_id {
                            idx = i;
                            break;
                        }
                    }
                    if idx < v.len() {
                        v.remove(idx);
                    }
                }
            }
        }
    }
}
