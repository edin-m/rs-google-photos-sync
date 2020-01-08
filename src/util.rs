use std::fs::File;
use std::io::BufReader;

use serde::de::{DeserializeOwned};
use serde_json;

use crate::error::CustomResult;
use std::cmp::min;

pub fn read_json_file<T>(path: String) -> CustomResult<T>
    where T: DeserializeOwned
{
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let parsed = serde_json::from_reader(reader)?;

    Ok(parsed)
}

pub fn split_into_groups<T>(items: &Vec<T>, group_size: usize) -> Vec<Vec<&T>>
{
    let mut groups = Vec::new();
    let num_groups = (items.len() as f32 / group_size as f32).ceil() as usize;

    for i in 0..num_groups {
        let mut vec = Vec::new();

        let start = i * group_size;
        let end = min(items.len(), i * group_size + group_size);

        for j in start..end {
            vec.push(items.get(j).unwrap());
        }

        groups.push(vec);
    }

    groups
}
