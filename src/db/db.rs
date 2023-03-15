use parking_lot::Mutex;
use sled;
use std::str;
use std::sync::Arc;

pub struct DbSession {
    db: Arc<Mutex<sled::Db>>,
}

impl DbSession {
    pub fn new(path: &'static str) -> DbSession {
        let db = sled::open(path).expect("open");
        db.clear().unwrap();
        let db = Arc::new(Mutex::new(db));
        DbSession { db }
    }

    pub fn set(&self, id: String, val: String) {

        self.db.lock().insert(id, val.as_str()).unwrap();
    }

    pub fn get(&self, id: String) -> String {
        let res = self.db.lock().get(id).unwrap();

        match res {
            None => "".to_string(),
            Some(val) => {
                let val = val.to_vec();
                String::from_utf8(val).unwrap().to_string()
            }
        }
    }
}

impl Clone for DbSession {
    fn clone(&self) -> Self {
        DbSession {
            db: self.db.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::db::db::DbSession;

    #[test]
    fn test_string() {
        let x: u32 = 10;

        let session = crate::db::db::DbSession::new("./data/db_test");
        session.set("key".to_string(), "val_redrock".to_string());
        let val: String = session.get("key".to_string());

        println!("{val}");
        assert_eq!(val, "val_redrock")
    }
}
