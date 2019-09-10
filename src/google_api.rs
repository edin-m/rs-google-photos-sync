use std::time;
use std::fs::File;
use std::io::{Write};
use std::option::Option;
use std::clone::Clone;
use std::thread;
use std::sync::mpsc;
use std::collections::HashMap;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json;

use chrono::{DateTime, Utc};

use nickel::{Nickel, HttpRouter, hyper::Url};

use opener;

use crate::util;
use crate::error::{CustomResult};

const CALLBACK_URL: &'static str = "http://localhost:3001/oauth2redirect";

pub struct GoogleAuthApi {
    credentials: GoogleCredentials,
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

//    pub fn get_token(&mut self) -> CustomResult<GoogleToken> {
//        self.authenticate_or_renew()?;
//
//        self.token.clone().ok_or(CustomError::Err("Could not clone token".to_string()))
//    }

    pub fn authenticate_or_renew(&mut self) -> CustomResult<GoogleToken> {
        if self.token.is_none() {
            let token = self.authenticate()?;
            self.token = Some(token);
        } else {
            if self.token.as_ref().unwrap().is_expired() {
                let token = self.renew_token()?;
                self.token = Some(token);
            }
        }

        Ok(self.token.clone().unwrap())
    }

    fn authenticate(&self) -> CustomResult<GoogleToken> {
        let url = create_authorization_url(&self.credentials.web);
        let code = get_authorization_code(url)?;

        println!("authorization code: {:#?}", code);

        let api_token = get_token(&self.credentials.web, code)?;
        println!("token {:#?}", api_token);

        let token = GoogleToken {
            token: api_token,
            token_created_at: Utc::now()
        };

        token.persist()?;

        Ok(token)
    }

    fn renew_token(&self) -> CustomResult<GoogleToken> {
        let mut token = self.token.clone().unwrap();

        let refresh_token = get_refresh_token(&self.credentials.web, &token.token)?;

        token.token.access_token = refresh_token.access_token;
        token.token.expires_in = refresh_token.expires_in;
        token.token_created_at = Utc::now();

        token.persist()?;

        Ok(token)
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

fn get_authorization_code(url: String) -> CustomResult<GoogleAuthorizationCode> {
    // open a web page to authorize
    opener::open(url)?;

    // pull authorization
    let thread = thread::spawn(move || {
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

    let item: HashMap<String, String> = thread.join()?;
    let code = item.get("code").unwrap();
    println!("thread result the code for access is: {}", code);

    Ok(GoogleAuthorizationCode(String::from(code)))
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

fn get_token(credentials: &GoogleWebCredentials, code: GoogleAuthorizationCode) -> CustomResult<GoogleApiToken> {
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

fn get_refresh_token(credentials: &GoogleWebCredentials, api_token: &GoogleApiToken) -> CustomResult<RefreshToken> {
    let token_request: HashMap<String, String> = build_refresh_token_request(
        &credentials, &api_token
    );

    println!("requiesting refresh token");
    let resp = reqwest_token::<RefreshToken>(&credentials.token_uri, token_request).unwrap();
    println!("resp {:#?}", resp);
    Ok(resp)
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

fn reqwest_token<T>(token_uri: &String, token_request: HashMap<String, String>) -> CustomResult<T>
    where T: DeserializeOwned
{
    let client = reqwest::Client::new();

    let resp = client.post(token_uri.as_str())
        .json(&token_request)
        .send()?.json()?;

    // NOTE: doesn't work with direct result return
    Ok(resp)
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
        let path = "secrets/credentials.json".to_string();

        let credentials = util::read_json_file::<GoogleCredentials>(path);

        match credentials {
            Ok(c) => c,
            Err(e) => {
                panic!("Error loading credentials file from secrets/credentials.json {}", e);
            }
        }
    }
}

impl StorageLoader for Option<GoogleToken> {
    fn read_stored() -> Self {
        let path = "secrets/token.json".to_string();

        util::read_json_file::<GoogleToken>(path).ok()
    }
}

trait Persistage {
    fn persist(&self) -> CustomResult<()>;
}

impl Persistage for GoogleToken {
    fn persist(&self) -> CustomResult<()> {
        let path = "secrets/token.json";
        let json: String = serde_json::to_string_pretty(&self)?;

        let mut file = File::create(path)?;
        file.write(json.as_bytes())?;
        file.sync_all()?;

        Ok(())
    }
}

