use std::fs::File;
use std::{thread, time};
use std::option::Option;
use std::sync::mpsc;
use std::collections::HashMap;
use std::string::ToString;

extern crate opener;

#[macro_use]
extern crate nickel;

extern crate reqwest;

#[macro_use]
extern crate serde;

#[macro_use]
extern crate serde_json;

#[macro_use]
extern crate serde_derive;

extern crate chrono;


use nickel::{Nickel, Mountable, Request, ListeningServer,
             HttpRouter, hyper::Url};

use reqwest::{Response};

use serde::{Deserialize, Serialize};
use serde_json::Result;

mod my_db;
mod google_api;
mod error;
mod util;

struct GoogleWebCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub redirect_uris: Vec<String>
}

struct GoogleAuthorizationCode(String);

#[derive(Deserialize)]
struct GoogleToken {
    pub access_token: String,
    pub expires_in: u32,
    pub refresh_token: String,
    pub scope: String,
    pub token_type: String
}

// =============

const CALLBACK_URL: &'static str = "http://localhost:3001/oauth2redirect";

fn main() {

    let mut x = google_api::GoogleAuthApi::create();
    let token = x.get_token();

    println!("{:#?}", token);

//    let credentials = parse_credentials();
//
//    println!("Hello, world!");
//    println!("client id {}", credentials.client_id);
//    println!("auth uri {}", credentials.auth_uri);
//    println!("token uri {}", credentials.token_uri);
//    println!("redirect uris {}", credentials.redirect_uris.len());

//    let url = create_authorization_url(&credentials);
//    println!("authorize uri {}", url);
//
//    let code = get_authorization_code(url);
//    println!("authroize code {}", code.0);
//
//    let token = get_token(&credentials, code);
//    println!("access token {}", token.access_token);
//    println!("refresh token {}", token.refresh_token);
}

//fn parse_credentials() -> GoogleWebCredentials {
//    let f = File::open("secrets/credentials.json").unwrap();
//    let json = ajson::parse_from_read(f).unwrap();
//
//    let client_id = json.get("web.client_id").unwrap();
//    let client_secret = json.get("web.client_secret").unwrap();
//    let auth_uri = json.get("web.auth_uri").unwrap();
//    let token_uri = json.get("web.token_uri").unwrap();
//    let items = json.get("web.redirect_uris").unwrap().to_vec();
//
//    let mut v: Vec<String> = Vec::new();
//    for i in &items {
//        v.push(i.to_string())
//    }
//
//    GoogleWebCredentials {
//        client_id: client_id.to_string(),
//        client_secret: client_secret.to_string(),
//        auth_uri: auth_uri.to_string(),
//        token_uri: token_uri.to_string(),
//        redirect_uris: v
//    }
//}

fn create_authorization_url(credentials: &GoogleWebCredentials) -> String {
    let scopes = google_photos_api_read_only_scope().join(" ");

    format!("{}?scope={}&response_type=code&redirect_uri={}&access_type=offline&client_id={}",
            credentials.auth_uri,
            scopes,
            CALLBACK_URL,
            credentials.client_id,
    )
}

fn google_photos_api_read_only_scope() -> Vec<String> {
    vec![
        String::from("https://www.googleapis.com/auth/photoslibrary.readonly")
    ]
}

fn get_authorization_code(url: String) -> GoogleAuthorizationCode {
    // open a web page to authorize
    opener::open(url);

    // pull authorization
    let thread = thread::spawn(move|| {
        let (tx, rx) = mpsc::sync_channel(1);

        let mut server = Nickel::new();

        server.get("/oauth2redirect", middleware! { |request|
                println!("{}", &request.origin.remote_addr);
                let query_params = parse_query_str(format!("{}", request.origin.uri));
                tx.send(query_params);
                "Authenticated"
        });

        let listener = server.listen("localhost:3001").unwrap();

        let params = rx.recv().unwrap();
        thread::sleep(time::Duration::from_secs(1));
        listener.detach();

        params
    });

    let item = thread.join().unwrap();
    println!("thread resutl the code for access is: {}", item.get("code").unwrap());

    GoogleAuthorizationCode(String::from(item.get("code").unwrap()))
}

fn parse_query_str(qstr: String) -> HashMap<String, String> {
    let mut query_params = HashMap::new();

    println!("{}", qstr);
    let parsed = Url::parse(format!("http://localhost{}", &qstr).as_str()).unwrap();

    for (k, v) in parsed.query_pairs() {
        println!("{} {}", k, v);
        query_params.insert(String::from(k), String::from(v));
    }

    query_params
}

fn get_token(credentials: &GoogleWebCredentials, code: GoogleAuthorizationCode) -> GoogleToken {
    let token_request: HashMap<String, String> = build_auth_token_request(
        &credentials, &code
    );

    reqwest_token(&credentials.token_uri, token_request)
}

fn build_auth_token_request(credentials: &GoogleWebCredentials,
                            code: &GoogleAuthorizationCode) -> HashMap<String, String> {
    let mut token_request = HashMap::new();

    token_request.insert("code".to_string(), code.0.to_string());
    token_request.insert("client_id".to_string(), credentials.client_id.to_string());
    token_request.insert("client_secret".to_string(), credentials.client_secret.to_string());
    token_request.insert("redirect_uri".to_string(), String::from(CALLBACK_URL));
    token_request.insert("grant_type".to_string(), String::from("authorization_code"));

    token_request
}

fn reqwest_token(token_uri: &String, token_request: HashMap<String, String>) -> GoogleToken {
    let client = reqwest::Client::new();

    client.post(token_uri.as_str())
        .json(&token_request)
        .send().unwrap().json().unwrap()
}
