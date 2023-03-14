use sled;
use std::str;

pub struct DbSession {
    db: sled::Db,
}

impl DbSession {
    pub fn new(path: &'static str) -> DbSession {
        let db = sled::open(path).expect("open");
        DbSession { db }
    }

    pub fn set(&self, id: String, val: String) {
        self.db.insert(id, val.as_str()).unwrap();
    }

    pub fn get(&self, id: String) -> String {
        let res = self.db.get(id).unwrap();

        match res {
            None => "".to_string(),
            Some(val) => {
                let val = val.to_vec();
                String::from_utf8(val).unwrap().to_string()
            }
        }
    }

    pub fn remove_all(&self) {
        self.db.clear().unwrap();
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
