use std::result;
use std::time;
use std::time::{Instant};
use std::fs::File;
use std::io::{BufReader, Write};
use std::option::Option;
use std::clone::Clone;
use std::marker::Copy;
use std::ops::Add;
use std::thread;
use std::sync::mpsc;
use std::collections::HashMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json;

use chrono::{DateTime, Utc, Duration};

use nickel::{Nickel, Mountable, Request, ListeningServer,
             HttpRouter, hyper::Url};

use reqwest::{Response};

use opener;

use crate::util;

const CALLBACK_URL: &'static str = "http://localhost:3001/oauth2redirect";

pub struct GoogleAuthApi {
    pub credentials: GoogleCredentials,
    pub token: Option<GoogleToken>,
}

#[derive(Deserialize, Debug)]
pub struct GoogleCredentials {
    web: GoogleWebCredentials
}

#[derive(Deserialize, Debug)]
pub struct GoogleWebCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub redirect_uris: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GoogleToken {
    pub token: GoogleApiToken,
    pub token_created_at: DateTime<Utc>,
}

impl GoogleToken {
    pub fn is_expired(&self) -> bool {
        let sec_from_token_creation = Utc::now().signed_duration_since(self.token_created_at).num_seconds();

        sec_from_token_creation > self.token.expires_in as i64
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GoogleApiToken {
    pub access_token: String,
    pub expires_in: u32,
    pub refresh_token: String,
    pub scope: String,
    pub token_type: String,
}

impl GoogleAuthApi {
    pub fn create() -> Self {
        let auth = GoogleAuthApi::read_stored();

        auth
    }

    pub fn get_token(&mut self) -> GoogleToken {
        self.normalize_token();

        self.token.clone().unwrap()
    }

    fn normalize_token(&mut self) {
        match &self.token {
            Some(token) => {
                if token.is_expired() {
                    println!("token expired, refreshing");
                    self.renew_token();
                }
            }
            None => {
                println!("Token non existent; authenticate");
                self.authenticate()
            }
        }
    }

    fn authenticate(&mut self) {
        let url = create_authorization_url(&self.credentials.web);
        let code = get_authorization_code(url);

        println!("authorization code: {:#?}", code);

        let token = get_token(&self.credentials.web, code);
        println!("token {:#?}", token);

        let google_token = GoogleToken {
            token,
            token_created_at: Utc::now()
        };

        google_token.persist();

        self.token = Some(google_token);
    }

    fn renew_token(&mut self) {
        let mut token = self.token.take().unwrap();

        let refresh_token = get_refresh_token(
            &self.credentials.web, &token.token,
        );

        token.token.access_token = refresh_token.access_token;
        token.token.expires_in = refresh_token.expires_in;
        token.token_created_at = Utc::now();

        token.persist();

        self.token = Some(token);
    }
}

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

#[derive(Debug)]
struct GoogleAuthorizationCode(String);

fn get_authorization_code(url: String) -> GoogleAuthorizationCode {
    // open a web page to authorize
    opener::open(url).unwrap();

    // pull authorization
    let thread = thread::spawn(move|| {
        let (tx, rx) = mpsc::sync_channel(1);

        let mut server = Nickel::new();

        server.get("/oauth2redirect", middleware! { |request|
                println!("{}", &request.origin.remote_addr);
                let query_params = parse_query_str(format!("{}", request.origin.uri));
                tx.send(query_params).unwrap();
                "Authenticated"
        });

        let listener = server.listen("localhost:3001").unwrap();

        let params = rx.recv().unwrap();
        thread::sleep(time::Duration::from_secs(1));
        listener.detach();

        params
    });

    let item = thread.join().unwrap();
    println!("thread result the code for access is: {}", item.get("code").unwrap());

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

fn get_token(credentials: &GoogleWebCredentials, code: GoogleAuthorizationCode) -> GoogleApiToken {
    let token_request: HashMap<String, String> = build_auth_token_request(
        &credentials, &code
    );

    reqwest_token::<GoogleApiToken>(&credentials.token_uri, token_request)
}

#[derive(Deserialize, Debug)]
struct RefreshToken {
    pub expires_in: u32,
    pub access_token: String,
    pub token_type: String
}

fn get_refresh_token(credentials: &GoogleWebCredentials, api_token: &GoogleApiToken) -> RefreshToken {
    let token_request: HashMap<String, String> = build_refresh_token_request(
        &credentials, &api_token
    );

    reqwest_token::<RefreshToken>(&credentials.token_uri, token_request)
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

fn build_refresh_token_request(credentials: &GoogleWebCredentials,
                               api_token: &GoogleApiToken,
) -> HashMap<String, String> {
    let mut refresh_request = HashMap::new();

    refresh_request.insert("refresh_token".to_string(), api_token.refresh_token.to_string());
    refresh_request.insert("client_id".to_string(), credentials.client_id.to_string());
    refresh_request.insert("client_secret".to_string(), credentials.client_secret.to_string());
    refresh_request.insert("grant_type".to_string(), "refresh_token".to_string());

    refresh_request
}

fn reqwest_token<T>(token_uri: &String, token_request: HashMap<String, String>) -> T
    where T: DeserializeOwned
{
    let client = reqwest::Client::new();

    println!("{:#?}", token_request);

    client.post(token_uri.as_str())
        .json(&token_request)
        .send().unwrap().json().unwrap()
}

trait StorageLoader {
    fn read_stored() -> Self;
}

impl StorageLoader for GoogleAuthApi {
    fn read_stored() -> Self {
        let credentials = GoogleCredentials::read_stored();
        let token = Option::<GoogleToken>::read_stored();

        GoogleAuthApi {
            credentials,
            token
        }
    }
}

impl StorageLoader for GoogleCredentials {
    fn read_stored() -> GoogleCredentials {
        let path = "secrets/credentials.json";

        util::read_json_file::<GoogleCredentials>(path.to_string())
    }
}

impl StorageLoader for Option<GoogleToken> {
    fn read_stored() -> Self {
        let path = "secrets/token.json";

        if let Ok(_) = File::open(path) {
            let token = util::read_json_file::<GoogleToken>(path.to_string());
            Some(token)
        } else {
            None
        }
    }
}

trait Persistage {
    fn persist(&self);
}

impl Persistage for GoogleToken {
    fn persist(&self) {
        let path = "secrets/token.json";
        let json: String = serde_json::to_string_pretty(&self).unwrap();

        let mut file = File::create(path).unwrap();
        file.write(json.as_bytes()).unwrap();
        file.sync_all().unwrap();
    }
}

