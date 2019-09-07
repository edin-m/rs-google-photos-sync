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
use std::collections::hash_map::Iter;
use std::iter::Filter;

use chrono::{DateTime, Utc};

use serde::{de, Serialize, Deserialize, de::DeserializeOwned};
use serde_json::{Value, Deserializer};
use serde_json;
use crate::error::CustomResult;

pub struct KeyValueStore<T> {
    pub data: Box<HashMap<String, T>>,
    pub path: String,
    pub last_save_at: DateTime<Utc>
}

impl<T> KeyValueStore<T>
    where T: Serialize + DeserializeOwned
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

    pub fn get(&self, key: &String) -> Option<&T>
    {
        self.data.get(key)
    }

    pub fn get_cloned(&self, key: &String) -> Option<T> {
        if let Some(item) = self.data.get(key) {
            let ser = serde_json::to_string(&item).expect("Could not convert item to json");
            Some(serde_json::from_str(&ser.to_string()).expect("Could not convert json to item"))
        } else {
            None
        }
    }

    pub fn set(&mut self, key: &String, t: T) -> Option<T>
    {
        let saved = self.data.insert(key.to_string(), t);

        if self.should_persist() {
            self.persist().expect("Could not perist key value store");
            self.last_save_at = Utc::now();
        }

        saved
    }

    fn should_persist(&self) -> bool {
        Utc::now().signed_duration_since(self.last_save_at).num_milliseconds() > 5000
    }

    pub fn filter_values<F>(&self, filter_fn: F) -> Vec<T>
        where F: Fn(&(&String, &T)) -> bool
    {
        let mut results = Vec::<T>::new();

        for (k, v) in self.data.iter().filter(filter_fn) {
            if let Some(cloned) = self.get_cloned(k) {
                results.push(cloned);
            }
        }

        results
    }

    pub fn load(&mut self) -> CustomResult<()> {
        let p = Path::new(&self.path);
        if p.exists() {
            let data = fs::read_to_string(&self.path)?;
            self.data = serde_json::from_str(&data)?;
        }

        println!("loaded {} stored items", self.data.len());

        Ok(())
    }

    pub fn persist(&self) -> CustomResult<()> {
        let serialized = serde_json::to_string_pretty(&self.data.as_ref())?;

        fs::write(&self.path, serialized)?;

        Ok(())
    }
}
