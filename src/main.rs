use std::fs::File;
use std::future;

extern crate ajson;
extern crate url;

use url::{Url, ParseError};

struct GoogleWebCredentials {
    pub client_id: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub redirect_uris: Vec<String>
}

fn main() {
    let credentials = parse_credentials();

    println!("Hello, world!");
    println!("client id {}", credentials.client_id);
    println!("auth uri {}", credentials.auth_uri);
    println!("token uri {}", credentials.token_uri);
    println!("redirect uris {}", credentials.redirect_uris.len());



}

fn parse_credentials() -> GoogleWebCredentials {
    let f = File::open("secrets/credentials.json").unwrap();
    let json = ajson::parse_from_read(f).unwrap();

    let client_id = json.get("web.client_id").unwrap();
    let auth_uri = json.get("web.auth_uri").unwrap();
    let token_uri = json.get("web.token_uri").unwrap();
    let items = json.get("web.redirect_uris").unwrap().to_vec();

    let mut v: Vec<String> = Vec::new();
    for i in &items {
        v.push(i.to_string())
    }

    GoogleWebCredentials {
        client_id: client_id.to_string(),
        auth_uri: auth_uri.to_string(),
        token_uri: token_uri.to_string(),
        redirect_uris: v
    }
}
