use std::result;
use std::time::Instant;
use std::fs::File;
use std::io::BufReader;
use std::option::Option;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json;

use chrono::{DateTime, Utc};

pub fn read_json_file<T>(path: String) -> T
    where T: DeserializeOwned
{
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).unwrap()
}
