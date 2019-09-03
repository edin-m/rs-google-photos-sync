use std::fs;
use std::collections::HashMap;
use std::result::Result;
use std::cmp::Eq;
use std::hash::Hash;
use std::marker::Copy;
use std::clone::Clone;
use std::fmt::Debug;
use std::boxed::Box;

use serde::{de, Serialize, Deserialize, de::DeserializeOwned};
use serde_json::{Value, Deserializer};
use serde_json;

type StringHashMap = HashMap<String, String>;

pub struct KeyValueStore {
    pub data: Box<StringHashMap>
}

impl KeyValueStore
{
    pub fn new() -> KeyValueStore {
        KeyValueStore {
            data: Box::new(StringHashMap::new())
        }
    }

    pub fn get<V>(&self, key: &String) -> Option<V>
        where V: DeserializeOwned
    {
        let item = self.data.get(key).unwrap();

        let deser: V = serde_json::from_str(&item.clone()).unwrap();

        Some(deser)
    }

    pub fn set<V>(&mut self, key: String, v: V) -> Option<V>
        where V: Serialize
    {
        let serialized = serde_json::to_string(&v).unwrap();

        self.data.insert(key, serialized);

        Some(v)
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
struct Point {
    pub x: i32,
    pub y: i32,
}

pub fn run_example() {
    fs::create_dir_all("db/my_db");

    let mut store = KeyValueStore::new();

    let x = Point { x: 100, y: 200 };

    let v = store.set("wicked".to_string(), x);
    println!("set {} {}", v.unwrap().x, v.unwrap().y);

    let v2: Point = store.get(&"wicked".to_string()).unwrap();
    println!("get {} {}", v2.x, v2.y);
}
