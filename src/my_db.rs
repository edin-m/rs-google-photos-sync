use std::fs;
use std::collections::HashMap;
use std::result::Result;
use std::cmp::Eq;
use std::hash::Hash;
use std::marker::Copy;
use std::clone::Clone;
use std::fmt::Debug;
use std::boxed::Box;
use std::path::Path;
use std::fs::File;
use std::io::Write;
use chrono::{DateTime, Utc};

use serde::{de, Serialize, Deserialize, de::DeserializeOwned};
use serde_json::{Value, Deserializer};
use serde_json;

type StringHashMap = HashMap<String, String>;

pub struct KeyValueStore<T> {
    pub data: Box<HashMap<String, T>>,
    pub path: String,
    pub last_save_at: DateTime<Utc>
}

impl<T> KeyValueStore<T>
    where T: Serialize + DeserializeOwned + Copy
{
    pub fn new(path: &str) -> KeyValueStore<T> {
        let mut store = KeyValueStore {
            data: Box::new(HashMap::new()),
            path: path.to_string(),
            last_save_at: Utc::now()
        };

        store.load();

        store
    }

    pub fn get(&self, key: String) -> Option<&T>
    {
        self.data.get(&key)
    }

    pub fn set(&mut self, key: String, t: T) -> Option<T>
    {
        let saved = self.data.insert(key, t);

        if self.should_persist() {
            self.persist();
        }

        saved
    }

    pub fn load(&mut self) {
        let p = Path::new(&self.path);
        if p.exists() {
            let data = fs::read_to_string(&self.path).unwrap();
            self.data = serde_json::from_str(&data).unwrap();
        }
    }

    pub fn persist(&self) {
        let serialized = serde_json::to_string_pretty(&self.data.as_ref()).unwrap();

        fs::write(&self.path, serialized);
    }

    fn should_persist(&self) -> bool {
        Utc::now().signed_duration_since(self.last_save_at).num_milliseconds() > 5000
    }
}

//#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
//struct Point {
//    pub x: i32,
//    pub y: i32,
//}

//pub fn run_example() {
//    fs::create_dir_all("db/my_db");
//
//    let mut store = KeyValueStore::new();
//
//    let x = Point { x: 100, y: 200 };
//
//    let v = store.set("wicked".to_string(), x);
//    println!("set {} {}", v.unwrap().x, v.unwrap().y);
//
//    let v2: Point = store.get(&"wicked".to_string()).unwrap();
//    println!("get {} {}", v2.x, v2.y);
//}
