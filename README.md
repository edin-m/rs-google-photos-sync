# rs-google-photos-sync

READ-ONLY sync Google Photos to local disk.

1. Create credentials.json from Google Console and add it to /secrets
2. ./rs-google-photos-sync -- runs jobs

Job configuration is in main.rs
Database is in secrets/photos.data
Download is in google/photos

CLI (non-cron job) modes:

* Run with ./rs-google-photos-sync [params]
* or
* cargo run -- [params]

* search: ./rs-google-photos-sync --search [search days back] [limit number]
* download: ./rs-google-photos-sync --download [limit number]

For first instance, run search to get all photos.
Then, cron should take the same.
In cron mode, it's searching 10 days back and downloading 10 files at a time.
This can be changed in main.rs
