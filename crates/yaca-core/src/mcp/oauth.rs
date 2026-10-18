//! OAuth for remote MCP servers: the login a server asks for with a 401.
//!
//! The protocol's authorization flow, as its specification lays it out for a
//! public client that runs on the person's machine:
//!
//! 1. Discovery. The server's protected-resource metadata (RFC 9728) names its
//!    authorization server — found where the 401's `WWW-Authenticate` points,
//!    or at the well-known address. The authorization server's metadata
//!    (RFC 8414, or OpenID's) gives its endpoints. Where either is missing,
//!    the specification's defaults on the server's own origin are used.
//! 2. Registration. Yaca registers itself as a client (RFC 7591) with the
//!    redirect it is about to listen on. A server that offers no registration
//!    cannot be logged in to from here, and says so.
//! 3. The login. A browser page at the authorization endpoint, with PKCE
//!    (S256) and the server named as the `resource` (RFC 8707). The answer
//!    comes back to a one-shot listener on 127.0.0.1.
//! 4. Tokens. The code is exchanged for an access token and, usually, a
//!    refresh token. Both are kept in the system keychain by the server's
//!    address; an access token past its expiry is refreshed before use.
//!
//! What is not here: a client registered once and kept (a fresh one per
//! login is what a loopback redirect on a new port needs anyway), and scopes
//! beyond what the 401 asks for.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// How long a lookup during the login may take.
const LOOKUP: Duration = Duration::from_secs(20);
/// An access token this close to its expiry is refreshed rather than sent.
const EXPIRY_MARGIN_SECS: u64 = 60;

/// Where tokens are kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum TokenStore {
    /// The system keychain, under Yaca's name. What the applications use.
    #[default]
    Keychain,
    /// One JSON file per server in a folder. For tests, which must not write
    /// to the keychain of the machine they run on.
    Dir(PathBuf),
}

/// What a login left behind for one server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Seconds since the epoch. `None` — the server did not say.
    #[serde(default)]
    pub expires_at: Option<u64>,
    pub token_endpoint: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
    /// The server the token is for (RFC 8707).
    pub resource: String,
}

impl TokenStore {
    fn account(url: &str) -> String {
        format!("mcp-oauth:{url}")
    }

    fn file(dir: &std::path::Path, url: &str) -> PathBuf {
        let name: String = url
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        dir.join(format!("{name}.json"))
    }

    pub fn load(&self, url: &str) -> Option<Grant> {
        let text = match self {
            TokenStore::Keychain => crate::secrets::read_secret(&Self::account(url))?,
            TokenStore::Dir(dir) => std::fs::read_to_string(Self::file(dir, url)).ok()?,
        };
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, url: &str, grant: &Grant) -> Result<(), String> {
        let text = serde_json::to_string(grant).map_err(|e| e.to_string())?;
        match self {
            TokenStore::Keychain => {
                crate::secrets::store_secret(&Self::account(url), &text).map_err(|e| e.to_string())
            }
            TokenStore::Dir(dir) => {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                std::fs::write(Self::file(dir, url), text).map_err(|e| e.to_string())
            }
        }
    }

    /// Logs out of `url`: the tokens are gone, here and in the keychain.
    pub fn forget(&self, url: &str) {
        match self {
            TokenStore::Keychain => crate::secrets::forget_secret(&Self::account(url)),
            TokenStore::Dir(dir) => {
                let _ = std::fs::remove_file(Self::file(dir, url));
            }
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(LOOKUP)
        // The redirects a login takes are the browser's; a lookup that
        // redirects is followed like any other.
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// An access token for `url` that can be sent now, refreshed first when it
/// is about to expire. `None` when there is no login, or it could not be
/// refreshed: the server will answer 401 and ask for one.
pub async fn bearer(store: &TokenStore, url: &str) -> Option<String> {
    let grant = store.load(url)?;
    let fresh = grant.expires_at.is_none_or(|at| now() + EXPIRY_MARGIN_SECS < at);
    if fresh {
        return Some(grant.access_token);
    }
    let refresh = grant.refresh_token.clone()?;
    let mut form = vec![
        ("grant_type", "refresh_token".to_string()),
        ("refresh_token", refresh),
        ("client_id", grant.client_id.clone()),
        ("resource", grant.resource.clone()),
    ];
    if let Some(secret) = &grant.client_secret {
        form.push(("client_secret", secret.clone()));
    }
    let answer = token_request(&grant.token_endpoint, &form).await.ok()?;
    let renewed = Grant {
        refresh_token: answer.refresh_token.or(grant.refresh_token),
        expires_at: answer.expires_in.map(|s| now() + s),
        access_token: answer.access_token,
        ..grant
    };
    let _ = store.save(url, &renewed);
    Some(renewed.access_token)
}

#[derive(Debug, Deserialize)]
struct TokenAnswer {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

async fn token_request(endpoint: &str, form: &[(&str, String)]) -> Result<TokenAnswer, String> {
    let response = client()
        .post(endpoint)
        .header("Accept", "application/json")
        .form(form)
        .send()
        .await
        .map_err(|e| format!("token endpoint: {e}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("token endpoint answered {}: {}", status.as_u16(), body.trim()));
    }
    serde_json::from_str(&body).map_err(|e| format!("token endpoint answer does not parse: {e}"))
}

/// The endpoints a login uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Endpoints {
    pub authorize: String,
    pub token: String,
    pub register: Option<String>,
}

/// A login under way: the page to open, and the rest of the flow to await.
pub struct Started {
    pub authorize_url: String,
    finish: Pin<Box<dyn Future<Output = Result<(), String>> + Send>>,
}

impl Started {
    /// Waits for the browser to come back, exchanges the code and keeps the
    /// tokens.
    pub async fn finish(self) -> Result<(), String> {
        self.finish.await
    }
}

/// Starts a login to the server at `server_url`. `challenge` is the 401's
/// `WWW-Authenticate`, which may say where to look.
pub async fn start(
    store: TokenStore,
    server_url: &str,
    challenge: Option<&str>,
) -> Result<Started, String> {
    let server = Url::parse(server_url).map_err(|e| format!("address does not parse: {e}"))?;
    let endpoints = discover(&server, challenge).await?;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("cannot listen for the login's answer: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}/callback");

    let register = endpoints.register.clone().ok_or_else(|| {
        "the server's authorization server offers no client registration, so Yaca cannot log in to it"
            .to_string()
    })?;
    let (client_id, client_secret) = register_client(&register, &redirect).await?;

    let verifier = format!("{}{}", random_hex(), random_hex());
    let state = random_hex();
    let scope = challenge.and_then(|c| auth_param(c, "scope"));
    let authorize_url = authorize_url(
        &endpoints.authorize,
        &client_id,
        &redirect,
        &challenge_of(&verifier),
        &state,
        server_url,
        scope.as_deref(),
    )?;

    let resource = server_url.to_string();
    let token_endpoint = endpoints.token.clone();
    let url = server_url.to_string();
    let finish = async move {
        let code = wait_for_code(&listener, &state).await?;
        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", code),
            ("redirect_uri", redirect),
            ("client_id", client_id.clone()),
            ("code_verifier", verifier),
            ("resource", resource.clone()),
        ];
        if let Some(secret) = &client_secret {
            form.push(("client_secret", secret.clone()));
        }
        let answer = token_request(&token_endpoint, &form).await?;
        store.save(
            &url,
            &Grant {
                access_token: answer.access_token,
                refresh_token: answer.refresh_token,
                expires_at: answer.expires_in.map(|s| now() + s),
                token_endpoint,
                client_id,
                client_secret,
                resource,
            },
        )
    };
    Ok(Started {
        authorize_url,
        finish: Box::pin(finish),
    })
}

/// Finds the endpoints: the resource's metadata, then its authorization
/// server's, then the specification's defaults.
pub(crate) async fn discover(server: &Url, challenge: Option<&str>) -> Result<Endpoints, String> {
    let http = client();
    let metadata_url = challenge
        .and_then(|c| auth_param(c, "resource_metadata"))
        .or_else(|| well_known(server, "oauth-protected-resource"));
    let mut issuer = None;
    if let Some(metadata_url) = metadata_url {
        if let Some(doc) = get_json(&http, &metadata_url).await {
            issuer = doc["authorization_servers"][0].as_str().map(str::to_string);
        }
    }
    let issuer = match issuer {
        Some(issuer) => Url::parse(&issuer).map_err(|e| format!("authorization server address: {e}"))?,
        None => origin(server),
    };

    for kind in ["oauth-authorization-server", "openid-configuration"] {
        let Some(address) = well_known(&issuer, kind) else { continue };
        if let Some(doc) = get_json(&http, &address).await {
            let field = |key: &str| doc[key].as_str().map(str::to_string);
            if let (Some(authorize), Some(token)) = (field("authorization_endpoint"), field("token_endpoint")) {
                return Ok(Endpoints {
                    authorize,
                    token,
                    register: field("registration_endpoint"),
                });
            }
        }
    }
    let base = origin(&issuer);
    let at = |path: &str| base.join(path).map(|u| u.to_string()).unwrap_or_default();
    Ok(Endpoints {
        authorize: at("/authorize"),
        token: at("/token"),
        register: Some(at("/register")),
    })
}

fn origin(url: &Url) -> Url {
    let mut base = url.clone();
    base.set_path("/");
    base.set_query(None);
    base.set_fragment(None);
    base
}

/// `https://host/.well-known/<kind><path>`: the path of the resource goes
/// after the well-known part (RFC 8414 §3, RFC 9728 §3).
fn well_known(url: &Url, kind: &str) -> Option<String> {
    let path = url.path().trim_end_matches('/');
    let mut at = origin(url);
    at.set_path(&format!("/.well-known/{kind}{path}"));
    Some(at.to_string())
}

async fn get_json(http: &reqwest::Client, url: &str) -> Option<Value> {
    let response = http.get(url).header("Accept", "application/json").send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json().await.ok()
}

async fn register_client(endpoint: &str, redirect: &str) -> Result<(String, Option<String>), String> {
    let response = client()
        .post(endpoint)
        .json(&json!({
            "client_name": "Yaca",
            "redirect_uris": [redirect],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
        }))
        .send()
        .await
        .map_err(|e| format!("client registration: {e}"))?;
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(format!("client registration answered {}: {body}", status.as_u16()));
    }
    let id = body["client_id"]
        .as_str()
        .ok_or("client registration gave no client_id")?
        .to_string();
    Ok((id, body["client_secret"].as_str().map(str::to_string)))
}

fn authorize_url(
    endpoint: &str,
    client_id: &str,
    redirect: &str,
    challenge: &str,
    state: &str,
    resource: &str,
    scope: Option<&str>,
) -> Result<String, String> {
    let mut url = Url::parse(endpoint).map_err(|e| format!("authorization endpoint: {e}"))?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state)
            .append_pair("resource", resource);
        if let Some(scope) = scope {
            query.append_pair("scope", scope);
        }
    }
    Ok(url.to_string())
}

/// PKCE's S256: the verifier's SHA-256, base64url without padding.
pub(crate) fn challenge_of(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn random_hex() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// One parameter of a `WWW-Authenticate: Bearer a="1", b="2"` header.
pub(crate) fn auth_param(header: &str, name: &str) -> Option<String> {
    let at = header.find(&format!("{name}="))?;
    let rest = &header[at + name.len() + 1..];
    let value = match rest.strip_prefix('"') {
        Some(quoted) => &quoted[..quoted.find('"')?],
        None => rest.split([',', ' ']).next()?,
    };
    (!value.is_empty()).then(|| value.to_string())
}

/// Serves the redirect until the browser comes back with `state`, and
/// returns the code. Anything else that knocks — a favicon, a stray tab — is
/// answered and ignored.
async fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String, String> {
    loop {
        let (mut socket, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut buffer = vec![0u8; 8192];
        let read = socket.read(&mut buffer).await.unwrap_or(0);
        let request = String::from_utf8_lossy(&buffer[..read]);
        let target = request.split_whitespace().nth(1).unwrap_or("/").to_string();
        let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else { continue };
        if url.path() != "/callback" {
            let _ = socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            continue;
        }
        let param = |key: &str| url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned());
        let outcome = if param("state").as_deref() != Some(state) {
            Err("the login's answer came back with a different state".to_string())
        } else if let Some(error) = param("error") {
            let about = param("error_description").unwrap_or_default();
            Err(format!("the login was refused: {error} {about}").trim().to_string())
        } else {
            param("code").ok_or_else(|| "the login's answer has no code".to_string())
        };
        let page = match &outcome {
            Ok(_) => "<html><body><h3>Yaca is logged in.</h3>You can close this tab.</body></html>",
            Err(_) => "<html><body><h3>Yaca could not log in.</h3>Go back to Yaca for the reason.</body></html>",
        };
        let _ = socket
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
                    page.len()
                )
                .as_bytes(),
            )
            .await;
        return outcome;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_the_rfcs_example() {
        // RFC 7636, appendix B.
        assert_eq!(
            challenge_of("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn www_authenticate_params_are_read_quoted_or_bare() {
        let header = r#"Bearer realm="mcp", resource_metadata="https://x.test/.well-known/oauth-protected-resource", scope=read"#;
        assert_eq!(
            auth_param(header, "resource_metadata").as_deref(),
            Some("https://x.test/.well-known/oauth-protected-resource")
        );
        assert_eq!(auth_param(header, "scope").as_deref(), Some("read"));
        assert_eq!(auth_param(header, "error"), None);
    }

    #[test]
    fn well_known_addresses_keep_the_path_after_them() {
        let url = Url::parse("https://api.example.test/v1/mcp").unwrap();
        assert_eq!(
            well_known(&url, "oauth-protected-resource").unwrap(),
            "https://api.example.test/.well-known/oauth-protected-resource/v1/mcp"
        );
        let root = Url::parse("https://auth.example.test").unwrap();
        assert_eq!(
            well_known(&root, "oauth-authorization-server").unwrap(),
            "https://auth.example.test/.well-known/oauth-authorization-server"
        );
    }

    #[test]
    fn the_login_page_carries_pkce_state_and_the_resource() {
        let url = authorize_url(
            "https://auth.test/authorize",
            "c1",
            "http://127.0.0.1:5000/callback",
            "abc",
            "s1",
            "https://mcp.test/mcp",
            Some("read"),
        )
        .unwrap();
        let parsed = Url::parse(&url).unwrap();
        let get = |k: &str| parsed.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.into_owned());
        assert_eq!(get("code_challenge_method").as_deref(), Some("S256"));
        assert_eq!(get("redirect_uri").as_deref(), Some("http://127.0.0.1:5000/callback"));
        assert_eq!(get("resource").as_deref(), Some("https://mcp.test/mcp"));
        assert_eq!(get("scope").as_deref(), Some("read"));
    }

    #[tokio::test]
    async fn a_fresh_token_is_sent_and_a_file_store_round_trips() {
        let dir = crate::testutil::TempDir::new();
        let store = TokenStore::Dir(dir.path().to_path_buf());
        let grant = Grant {
            access_token: "a1".into(),
            refresh_token: None,
            expires_at: Some(now() + 3600),
            token_endpoint: "https://auth.test/token".into(),
            client_id: "c".into(),
            client_secret: None,
            resource: "https://mcp.test/mcp".into(),
        };
        store.save("https://mcp.test/mcp", &grant).unwrap();
        assert_eq!(bearer(&store, "https://mcp.test/mcp").await.as_deref(), Some("a1"));
        store.forget("https://mcp.test/mcp");
        assert_eq!(bearer(&store, "https://mcp.test/mcp").await, None);
    }

    #[tokio::test]
    async fn an_expired_token_without_a_refresh_is_no_token() {
        let dir = crate::testutil::TempDir::new();
        let store = TokenStore::Dir(dir.path().to_path_buf());
        let grant = Grant {
            access_token: "old".into(),
            refresh_token: None,
            expires_at: Some(1),
            token_endpoint: "https://auth.test/token".into(),
            client_id: "c".into(),
            client_secret: None,
            resource: "r".into(),
        };
        store.save("u", &grant).unwrap();
        assert_eq!(bearer(&store, "u").await, None);
    }
}
