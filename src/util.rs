use std::result;
use std::time::Instant;
use std::fs::File;
use std::io::BufReader;
use std::option::Option;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json;

use chrono::{DateTime, Utc};
use serde::export::fmt::Debug;
use crate::error::CustomResult;

pub fn read_json_file<T>(path: String) -> CustomResult<T>
    where T: DeserializeOwned
{
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let parsed = serde_json::from_reader(reader)?;

    Ok(parsed)
}
