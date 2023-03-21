pub mod db;
pub mod thread_socket;

use crate::db::db::DbSession;
use crate::thread_socket::thread_socket::{new_socket, ThreadSocket};
use async_std::task::JoinHandle;
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use std::collections::VecDeque;
use std::{sync::Arc, thread};

#[derive(Debug)]
pub enum Event {
    RemoteRead(String),
    RemoteWrite(String),
    Confirmed(bool),
}

impl Event {
    fn get_id(&self) -> &String {
        match self {
            Event::RemoteRead(id) => id,
            Event::RemoteWrite(id) => id,
            Event::Confirmed(_) => panic!("Confirmed event has no id"),
        }
    }
}

impl Clone for Event {
    fn clone(&self) -> Self {
        match self {
            Event::RemoteRead(id) => Event::RemoteRead(id.clone()),
            Event::RemoteWrite(id) => Event::RemoteWrite(id.clone()),
            Event::Confirmed(b) => Event::Confirmed(b.clone()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Status {
    Modified,
    Exclusive,
    Shared,
    Invalid,
}

impl Clone for Status {
    fn clone(&self) -> Self {
        match self {
            Status::Modified => Status::Modified,
            Status::Exclusive => Status::Exclusive,
            Status::Shared => Status::Shared,
            Status::Invalid => Status::Invalid,
        }
    }
}

type ThreadID = usize;

const CACHE_SIZE: usize = 1 << 10;
const FLUSH_SIZE: usize = 1 << 7;

#[derive(Clone)]
pub struct CacheController<T: Clone + ToString + Sync> {
    caches: Arc<DashMap<String, Cache<T>>>,
    directory: Arc<RwLock<Directory>>,
    pub thread_id: ThreadID,

    // 一些测试指标
    op_cnt: u32,
    // 总操作次数
    in_cache_cnt: u32, // 缓存命中次数
}

impl<T: Clone + ToString + Sync + From<String> + 'static> CacheController<T> {
    pub fn new(directory: Arc<RwLock<Directory>>) -> CacheController<T> {
        let caches: Arc<DashMap<String, Cache<T>>> = Arc::new(DashMap::new());

        let mut directory = directory.clone();
        let (thread_id, socket) = directory.write().register();

        // 启动一个线程，监听来自 bus_line 的消息
        let t_id = thread_id.clone();
        let mut _caches = caches.clone();
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

                let mut is_invalid = false;
                match _caches.get_mut(id) {
                    None => is_invalid = true,
                    Some(mut cache) => {
                        if cache.status == Status::Modified {
                            _directory
                                .read()
                                .write_back(cache.id.clone(), cache.value.clone());
                        }
                        is_invalid = cache.handle(&event);
                    }
                };
                if is_invalid {
                    _caches.remove(id);
                }

                socket.send(Event::Confirmed(is_invalid));

                if _caches.len() < CACHE_SIZE {
                    continue;
                }

                println!("thread {:?} flushed caches", t_id.clone());

                let mut c = FLUSH_SIZE as i32;
                _caches.retain(|k, v| {
                    c -= 1;
                    c > 0
                });
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

    pub fn get(&mut self, id: String) -> T {
        self.op_cnt += 1;

        {
            // 命中缓存
            let mut cache = self.caches.get(&id);
            if matches!(&cache, Some(c) if c.status != Status::Invalid) {
                self.in_cache_cnt += 1;
                return cache.unwrap().value.clone();
            }
            // 释放读锁
        }

        let (v, n): (T, usize) = self
            .directory
            .read()
            .read(self.thread_id.clone(), id.clone());
        let status = if n > 0 {
            Status::Shared
        } else {
            Status::Exclusive
        };
        self.caches
            .insert(id.clone(), Cache::new(id, v.clone(), status));
        v
    }

    pub fn set(&mut self, id: String, val: T) {
        // 预处理
        self.op_cnt += 1;

        self.directory
            .read()
            .write_to_cache(self.thread_id.clone(), id.clone());

        match self.caches.get_mut(&id) {
            None => {}
            Some(mut c) => {
                self.in_cache_cnt += 1;
                c.status = Status::Modified;
                c.value = val;
                return;
            }
        }

        self.caches
            .insert(id.clone(), Cache::new(id, val.clone(), Status::Modified));
    }

    pub fn collect(&self) -> (u32, u32) {
        (self.in_cache_cnt.clone(), self.op_cnt.clone())
    }

    pub fn collect_caches(&self) -> Arc<DashMap<String, Cache<T>>> {
        self.caches.clone()
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

/// `Directory` 缓存目录
/// 使用实现了 shard 特性的 DashMap 提高系统并发度
pub struct Directory {
    map: DashMap<String, VecDeque<ThreadID>>,
    sockets: Mutex<Vec<ThreadSocket<Event>>>,
    db: DbSession,
}

impl Directory {
    pub fn new(db_path: &String) -> Directory {
        let map = DashMap::new();
        let sockets = Mutex::new(Vec::new());
        let db = DbSession::new(db_path);

        Directory { map, sockets, db }
    }

    pub fn register(&mut self) -> (ThreadID, ThreadSocket<Event>) {
        let (s1, s2) = new_socket();
        let mut sockets = self.sockets.lock();
        sockets.push(s1);

        let id = (sockets.len() - 1) as ThreadID;

        (id, s2)
    }

    fn broadcast(
        &self,
        thread_id: ThreadID,
        event: Event,
        ids: &VecDeque<ThreadID>,
    ) -> Option<Vec<ThreadID>> {
        if ids.len() == 0 {
            return None;
        }

        // println!("directory will broadcast event: {:?} from {:?}", event, thread_id);

        let sockets = self.sockets.lock();
        for i in ids {
            if *i == thread_id {
                continue;
            }
            sockets[*i].send(event.clone());
        }
        let mut invalid_ids = Vec::new();
        for i in ids {
            if *i == thread_id {
                continue;
            }
            if matches!(sockets[*i].receive(), Event::Confirmed(b) if b) {
                invalid_ids.push(i.clone());
            }
        }
        if invalid_ids.len() == 0 {
            return None;
        };
        Some(invalid_ids)
    }

    // 从 db 读取数据
    fn read<T: Clone + Sync + From<String>>(&self, thread_id: ThreadID, id: String) -> (T, usize) {
        let mut v = self.map.entry(id.clone()).or_insert(VecDeque::new());
        if v.len() > 0 {
            let ids = self.broadcast(thread_id, Event::RemoteRead(id.clone()), v.value());
            match ids {
                None => {}
                Some(ids) => {
                    let v = v.value_mut();
                    let mut idx = v.len();
                    for id in ids {
                        for i in 0..v.len() {
                            if v[i] == id {
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
        };

        v.value_mut().push_back(thread_id.clone());
        (self.db.get(id).into(), v.len() - 1)
        // {
        //     None => {}
        //     Some(mut v) => {
        //         let ids = self.broadcast(thread_id, Event::RemoteRead(id.clone()), v.value());
        //         match ids {
        //             None => {},
        //             Some(ids) => {
        //                 let v = v.value_mut();
        //                 let mut idx = v.len();
        //                 for id in ids {
        //                     for i in 0..v.len() {
        //                         if v[i] == id {
        //                             idx = i;
        //                             break;
        //                         }
        //                     }
        //                     if idx < v.len() { v.remove(idx); }
        //                 }
        //             }
        //         }
        //         v.value_mut().push_back(thread_id.clone());
        //         return (self.db.get(id).into(), v.len());
        //     }
        // };
        //
        // // 没有其他线程持有缓存，不需要广播
        // self.map.insert(id.clone(), VecDeque::from(vec![thread_id]));
        // (self.db.get(id).into(), 0)
    }

    // 将数据写回 db，
    fn write_back<T: Clone + Sync + ToString>(&self, id: String, val: T) {
        self.db.set(id, val.to_string());
    }

    // 维护目录，并广播msg
    fn write_to_cache(&self, thread_id: ThreadID, id: String) {
        // 更新目录
        let mut v = self.map.entry(id.clone()).or_insert(VecDeque::new());
        if v.value().len() > 0 {
            self.broadcast(thread_id, Event::RemoteWrite(id.clone()), v.value());
            v.value_mut().clear();
        };
        v.value_mut().push_back(thread_id.clone());
        // match self.map.get_mut(&*id.clone()) {
        //     None => {}
        //     Some(mut v) => {
        //         let v = v.value_mut();
        //         // 广播 message
        //         self.broadcast(thread_id, Event::RemoteWrite(id.clone()), v);
        //
        //         while !v.is_empty() && *(v.front().unwrap()) != thread_id {
        //             v.pop_front();
        //         }
        //         while !v.is_empty() && *(v.back().unwrap()) != thread_id {
        //             v.pop_back();
        //         }
        //         v.push_back(thread_id);
        //         return;
        //     }
        // };
        //
        // self.map.insert(id.clone(), VecDeque::from(vec![thread_id]));
    }
}

#[derive(Debug)]
pub struct Cache<T: Clone + ToString + Sync> {
    pub id: String,
    pub value: T,
    pub status: Status,
}

impl<T: Clone + ToString + Sync> Cache<T> {
    pub fn new(id: String, value: T, status: Status) -> Cache<T> {
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
            Event::Confirmed(_) => panic!("Cache can not handle confirmed event"),
        }
    }
}

unsafe impl<T: Clone + ToString + Sync> Send for Cache<T> {}
