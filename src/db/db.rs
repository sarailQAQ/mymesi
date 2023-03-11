use std::fmt::Display;
use std::str;
use sled;

pub struct DbSession {
    path: &'static str,
    db: sled::Db,
}

impl DbSession {
    pub fn new(path: &'static str) -> DbSession {
        let db = sled::open(path).expect("open");
        DbSession{ path, db }
    }

    pub fn set<T: ToString >(&self, id: String, val: T) {
        self.db.insert(id, val.to_string().as_str()).unwrap();
    }

    pub fn get<T: From<String>  >(self, id: String) -> T {
        let mut res = self.db.get(id).unwrap().unwrap();

        // println!("{:#?}", res);
        let res = res.to_vec();
        let res = String::from_utf8(res).unwrap();
        T::from(res)
    }

}

#[cfg(test)]
mod tests {
    use crate::db::db::DbSession;

    #[test]
    fn test_string() {
        let x: u32 = 10;

        let session = crate::db::db::DbSession::new("./data/db_test");
        session.set("key".to_string(), "val_redrock");
        let val: String = session.get("key".to_string());


        println!("{val}");
        assert_eq!(val, "val_redrock")

    }

}