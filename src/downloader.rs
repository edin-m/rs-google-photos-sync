use std::fs;
use std::fs::File;
use std::io::copy;
use std::path::Path;
use std::sync::mpsc;

use scoped_threadpool::Pool;

use crate::{MediaItemId, StoredItem};
use crate::error::{CustomResult};

pub fn download(stored_items: &Vec<StoredItem>) -> CustomResult<Vec<MediaItemId>> {
    fs::create_dir_all("google/photos")?;

    let group_size = 5;
    let mut pool = Pool::new(group_size);

    let (tx, rx) = mpsc::channel();

    pool.scoped(|scoped| {
        for stored_item in stored_items {
            let tx = tx.clone();
            scoped.execute(move || {
                let res = stored_item.download();

                match res {
                    Ok(_) => tx.send(Some(stored_item.mediaItem.id.to_owned())).unwrap(),
                    Err(e) => {
                        println!("Error downloading {:#?}", e);
                        tx.send(None).unwrap();
                    }
                }
            });
        }
    });

    let vec = rx.into_iter()
        .take(stored_items.len())
        .filter(|value| {
            match value {
                Some(_) => true,
                None => false
            }
        })
        .collect::<Option<Vec<_>>>();

    Ok(vec.or_else(|| Some(Vec::new())).unwrap())
}


pub trait Download {
    fn download(&self) -> CustomResult<()>;
}

pub trait DownloadUrl {
    fn create_download_url(&self) -> CustomResult<String>;
}

impl Download for StoredItem {
    fn download(&self) -> CustomResult<()> {
        let filename = self.get_filename();

        let url = self.mediaItem.create_download_url()?;

        let mut resp = reqwest::get(url.as_str())?;
        let path = Path::new("google/photos").join(&filename);
        println!("downloading {}", filename);
        let mut dest = File::create(path)?;

        let _ = copy(&mut resp, &mut dest)?;

        Ok(())
    }
}

