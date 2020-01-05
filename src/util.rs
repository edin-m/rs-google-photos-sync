use std::fs::File;
use std::io::BufReader;

use serde::de::{DeserializeOwned};
use serde_json;

use crate::error::CustomResult;

pub fn read_json_file<T>(path: String) -> CustomResult<T>
    where T: DeserializeOwned
{
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let parsed = serde_json::from_reader(reader)?;

    Ok(parsed)
}
