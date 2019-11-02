# rs-google-photos-sync

### Sync Google Photos from Web to Disk (**READ-ONLY**)

How to set up:
1. Create credentials.json on Google Console.
2. Add it to the /secrets folder.
3. Run `./rs-google-photos-sync` from command line.

It works by running scheduled jobs to extend auth token and to download new images available.

```
$ ./rs-google-photos-sync --help
Usage:
Read-only sync Google Photos onto a local disk

Options:
  -v, --version               Show the bin version and build time
  -h, --help                  Show this help message and exit
  -s, --search                [days back] [limit] Search and store media items
  -d, --download              [num files] Download media items
```

Job configuration is in main.rs.
Database is in secrets/photos.data.
Download is in google/photos.

CLI (non-cron job) modes:

* Run with ./rs-google-photos-sync [params] or
* cargo run -- [params]

* search: ./rs-google-photos-sync --search [search days back] [limit number]
* download: ./rs-google-photos-sync --download [limit number]

For first instance, run search to get all photos.

Then, cron should take the same.

In cron mode, it's searching 10 days back and downloading 10 files at a time.

This can be changed in main.rs

Duplicate filenames are prefixed with 0_ 1_ 2_ ...

TODO:
 * windows filetime not working properly
