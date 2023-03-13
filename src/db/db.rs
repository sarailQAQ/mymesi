use sled;
use std::str;

pub struct DbSession {
    path: &'static str,
    db: sled::Db,
}

impl DbSession {
    pub fn new(path: &'static str) -> DbSession {
        let db = sled::open(path).expect("open");
        DbSession { path, db }
    }

    pub fn set(&self, id: String, val: String) {
        self.db.insert(id, val.as_str()).unwrap();
    }

    pub fn get(&self, id: String) -> String {
        let res = self.db.get(id).unwrap().unwrap();

        // println!("{:#?}", res);
        let res = res.to_vec();
        String::from_utf8(res).unwrap()
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
