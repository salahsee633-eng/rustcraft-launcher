//! Microsoft / Xbox Live / Minecraft Services authentication.
//!
//! This is the same OAuth device-code flow PojavLauncher and other third-party
//! launchers use. You must register your own Azure application (public client,
//! "Mobile and desktop applications" platform) and put its client id below /
//! in the UI. Never ship a client secret in a mobile app.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::download::http_client;
use crate::error::{Error, Result};
use crate::session::Session;

pub const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
pub const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
pub const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
pub const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
pub const MC_LOGIN_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
pub const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// Replace with your own Azure application id.
pub const PLACEHOLDER_CLIENT_ID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
    #[serde(default)]
    pub message: String,
}

fn default_interval() -> u64 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    #[serde(default)]
    pub obtained_at: u64,
}

impl MsTokens {
    pub fn is_expired(&self) -> bool {
        // 60s safety margin.
        now_unix().saturating_sub(self.obtained_at) + 60 >= self.expires_in
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct XstsResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Debug, Clone, Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Debug, Clone, Deserialize)]
struct Xui {
    #[serde(default)]
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Step 1: ask Microsoft for a code the user types on microsoft.com/link.
pub fn start_device_login(client_id: &str) -> Result<DeviceCode> {
    let client = http_client()?;
    let res = client
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", client_id), ("scope", "XboxLive.signin offline_access")])
        .send()?;
    let status = res.status();
    let text = res.text()?;
    if !status.is_success() {
        return Err(Error::Auth(format!("device code request failed ({status}): {text}")));
    }
    Ok(serde_json::from_str(&text)?)
}

/// Step 2: poll until the user finishes logging in.
///
/// `on_tick` is called while waiting so the UI can keep showing the code.
pub fn wait_for_device_login(
    client_id: &str,
    code: &DeviceCode,
    mut on_tick: impl FnMut(&DeviceCode, u64),
) -> Result<MsTokens> {
    let client = http_client()?;
    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = code.interval.max(1);

    loop {
        if Instant::now() >= deadline {
            return Err(Error::Auth("the login code expired, please try again".into()));
        }
        on_tick(code, interval);
        std::thread::sleep(Duration::from_secs(interval));

        let res = client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client_id),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", code.device_code.as_str()),
            ])
            .send()?;
        let status = res.status();
        let text = res.text()?;
        if status.is_success() {
            let mut tokens: MsTokens = serde_json::from_str(&text)?;
            tokens.obtained_at = now_unix();
            return Ok(tokens);
        }

        let error = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_default();
        match error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => {
                interval += 5;
                continue;
            }
            "authorization_declined" => {
                return Err(Error::Auth("login was declined".into()))
            }
            "expired_token" => {
                return Err(Error::Auth("the login code expired".into()))
            }
            _ => return Err(Error::Auth(format!("token request failed ({status}): {text}"))),
        }
    }
}

/// Refresh an expired Microsoft token without asking the user again.
pub fn refresh_tokens(client_id: &str, refresh_token: &str) -> Result<MsTokens> {
    let client = http_client()?;
    let res = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()?;
    let status = res.status();
    let text = res.text()?;
    if !status.is_success() {
        return Err(Error::Auth(format!("token refresh failed ({status}): {text}")));
    }
    let mut tokens: MsTokens = serde_json::from_str(&text)?;
    tokens.obtained_at = now_unix();
    Ok(tokens)
}

/// Full chain: Microsoft token -> Xbox Live -> XSTS -> Minecraft services.
pub fn login_with_ms_tokens(tokens: &MsTokens) -> Result<Session> {
    let (xbl_token, _uhs) = xbox_live(&tokens.access_token)?;
    let (xsts_token, uhs, xuid) = xsts(&xbl_token)?;
    let mc_token = minecraft_login(&xsts_token, &uhs)?;
    let profile = minecraft_profile(&mc_token)?;

    Ok(Session {
        username: profile.name,
        uuid: profile.id,
        access_token: mc_token,
        user_type: "msa".to_string(),
        xuid: xuid.or(Some(uhs)),
        client_id: None,
    })
}

pub fn login_device_code(
    client_id: &str,
    on_tick: impl FnMut(&DeviceCode, u64),
) -> Result<Session> {
    let code = start_device_login(client_id)?;
    let tokens = wait_for_device_login(client_id, &code, on_tick)?;
    let mut session = login_with_ms_tokens(&tokens)?;
    session.client_id = Some(client_id.to_string());
    Ok(session)
}

fn xbox_live(ms_access_token: &str) -> Result<(String, String)> {
    let body = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    let value = post_json(XBL_URL, &body)?;
    let token = value
        .get("Token")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Auth("Xbox Live returned no token".into()))?
        .to_string();
    let uhs = value
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok((token, uhs))
}

fn xsts(xbl_token: &str) -> Result<(String, String, Option<String>)> {
    let body = json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });
    let client = http_client()?;
    let res = client.post(XSTS_URL).json(&body).send()?;
    let status = res.status();
    let text = res.text()?;

    if !status.is_success() {
        let xerr = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|v| v.get("XErr").and_then(Value::as_u64));
        let hint = match xerr {
            Some(2148916233) => {
                "this Microsoft account has no Xbox profile yet: create one at xbox.com"
            }
            Some(2148916235) => "Xbox Live is not available in this account's country",
            Some(2148916238) => {
                "this is a child account: an adult must add it to a Microsoft family"
            }
            _ => "XSTS authorization failed",
        };
        return Err(Error::Auth(format!("{hint} ({status}): {text}")));
    }

    let xsts: XstsResponse = serde_json::from_str(&text)?;
    let xui = xsts
        .display_claims
        .xui
        .first()
        .ok_or_else(|| Error::Auth("XSTS returned no user claims".into()))?;
    Ok((xsts.token, xui.uhs.clone(), xui.xid.clone()))
}

fn minecraft_login(xsts_token: &str, uhs: &str) -> Result<String> {
    let body = json!({
        "identityToken": format!("XBL3.0 x={uhs};{xsts_token}")
    });
    let client = http_client()?;
    let res = client.post(MC_LOGIN_URL).json(&body).send()?;
    let status = res.status();
    let text = res.text()?;
    if !status.is_success() {
        return Err(Error::Auth(format!(
            "Minecraft login failed ({status}): {text}"
        )));
    }
    let value: Value = serde_json::from_str(&text)?;
    value
        .get("access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::Auth("Minecraft returned no access token".into()))
}

fn minecraft_profile(mc_token: &str) -> Result<MinecraftProfile> {
    let client = http_client()?;
    let res = client
        .get(MC_PROFILE_URL)
        .bearer_auth(mc_token)
        .send()?;
    let status = res.status();
    let text = res.text()?;
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::Auth(
            "this account does not own Minecraft: Java Edition".into(),
        ));
    }
    if !status.is_success() {
        return Err(Error::Auth(format!(
            "profile request failed ({status}): {text}"
        )));
    }
    Ok(serde_json::from_str(&text)?)
}

fn post_json(url: &str, body: &Value) -> Result<Value> {
    let client = http_client()?;
    let res = client.post(url).json(body).send()?;
    let status = res.status();
    let text = res.text()?;
    if !status.is_success() {
        return Err(Error::Auth(format!("request to {url} failed ({status}): {text}")));
    }
    Ok(serde_json::from_str(&text)?)
}
