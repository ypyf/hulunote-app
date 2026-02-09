mod components;

use crate::components::ui::{
    Alert, AlertDescription, Button, ButtonSize, ButtonVariant, Card, CardContent, CardDescription,
    CardHeader, CardTitle, Input, Label, Spinner,
};
use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_dom::helpers::window_event_listener;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::{use_navigate, use_query_map};
use leptos_router::params::Params;
use leptos_router::path;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::sync::Arc;
use wasm_bindgen::JsCast;

// Needed for `#[wasm_bindgen(start)]` on the wasm entrypoint.
#[cfg(all(target_arch = "wasm32", not(test)))]
use wasm_bindgen::prelude::wasm_bindgen;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EnvConfig {
    pub api_url: String,
}

impl EnvConfig {
    pub fn new() -> Self {
        let default_api_url = "http://localhost:6689".to_string();

        // We support BOTH `window.ENV.API_URL` (documented in README) and
        // `window.ENV.api_url` (legacy/implementation detail) for compatibility.
        if let Some(window) = web_sys::window() {
            if let Some(env) = window.get("ENV") {
                if !env.is_undefined() && env.is_object() {
                    // 1) Prefer README style: API_URL
                    if let Ok(api_url) = js_sys::Reflect::get(&env, &"API_URL".into()) {
                        if let Some(url_str) = api_url.as_string() {
                            return Self { api_url: url_str };
                        }
                    }

                    // 2) Fallback: api_url
                    if let Ok(api_url) = js_sys::Reflect::get(&env, &"api_url".into()) {
                        if let Some(url_str) = api_url.as_string() {
                            return Self { api_url: url_str };
                        }
                    }
                }
            }
        }

        Self {
            api_url: default_api_url,
        }
    }
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self::new()
    }
}

fn get_api_url() -> String {
    EnvConfig::new().api_url
}

/// Backend account info object.
///
/// hulunote-rust returns this under the `hulunote` field.
/// We keep it flexible to avoid breaking when backend fields evolve.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AccountInfo {
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginResponse {
    pub token: String,
    pub hulunote: AccountInfo,
    pub region: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Database {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Note {
    pub id: String,
    pub database_id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Nav {
    pub id: String,
    pub note_id: String,
    pub parent_id: Option<String>,
    pub content: String,
    pub position: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateDatabaseRequest {
    // hulunote-rust expects kebab-case keys.
    #[serde(rename = "database-name")]
    pub database_name: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateDatabaseRequest {
    // Backend accepts `database-id` or `id`.
    #[serde(rename = "database-id", skip_serializing_if = "Option::is_none")]
    pub database_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    // Backend uses `db-name` for rename.
    #[serde(rename = "db-name", skip_serializing_if = "Option::is_none")]
    pub db_name: Option<String>,

    #[serde(rename = "is-public", skip_serializing_if = "Option::is_none")]
    pub is_public: Option<bool>,
    #[serde(rename = "is-default", skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    #[serde(rename = "is-delete", skip_serializing_if = "Option::is_none")]
    pub is_delete: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeleteDatabaseRequest {
    #[serde(rename = "database-id", skip_serializing_if = "Option::is_none")]
    pub database_id: Option<String>,
    #[serde(rename = "database-name", skip_serializing_if = "Option::is_none")]
    pub database_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateNoteRequest {
    pub database_id: String,
    pub title: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GetNoteListRequest {
    pub database_id: String,
    pub page: i32,
    pub page_size: i32,
}

fn today_yyyymmdd_local() -> String {
    // Use system local timezone (browser runtime).
    let d = js_sys::Date::new_0();
    let y = d.get_full_year();
    let m = d.get_month() + 1;
    let day = d.get_date();
    format!("{:04}{:02}{:02}", y, m, day)
}

fn next_available_daily_note_title_for_date(base: &str, existing_notes: &[Note]) -> String {
    let base = base.trim();

    let mut has_base = false;
    let mut max_suffix: u32 = 1;

    for n in existing_notes {
        let t = n.title.trim();
        if t == base {
            has_base = true;
            continue;
        }

        // Match patterns like: YYYYMMDD-2, YYYYMMDD-3, ...
        if let Some(rest) = t.strip_prefix(&format!("{}-", base)) {
            if let Ok(k) = rest.parse::<u32>() {
                if k >= max_suffix {
                    max_suffix = k;
                }
            }
        }
    }

    if !has_base {
        return base.to_string();
    }

    format!("{}-{}", base, max_suffix.saturating_add(1))
}

fn next_available_daily_note_title(existing_notes: &[Note]) -> String {
    next_available_daily_note_title_for_date(&today_yyyymmdd_local(), existing_notes)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignupRequest {
    pub email: String,

    /// Some hulunote backends expect a `username` string; older clients sometimes pass email here.
    pub username: Option<String>,

    pub password: String,

    /// Registration/invite code.
    pub registration_code: String,

    /// Optional fields used by some deployed backends (see legacy client).
    #[serde(rename = "ack-number", skip_serializing_if = "Option::is_none")]
    pub ack_number: Option<String>,

    #[serde(rename = "binding-code", skip_serializing_if = "Option::is_none")]
    pub binding_code: Option<String>,

    #[serde(rename = "binding-platform", skip_serializing_if = "Option::is_none")]
    pub binding_platform: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignupResponse {
    pub token: String,
    pub hulunote: AccountInfo,
    pub database: Option<String>,
    pub region: Option<String>,
}

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    token: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            token: None,
        }
    }

    pub fn load_from_storage() -> Self {
        let base_url = get_api_url();
        let token = leptos::web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(TOKEN_KEY).ok().flatten());

        Self { base_url, token }
    }

    pub fn save_to_storage(&self) {
        if let Some(storage) =
            leptos::web_sys::window().and_then(|w| w.local_storage().ok().flatten())
        {
            if let Some(token) = &self.token {
                let _ = storage.set_item(TOKEN_KEY, token);
            }
        }
    }

    pub fn clear_storage() {
        if let Some(storage) =
            leptos::web_sys::window().and_then(|w| w.local_storage().ok().flatten())
        {
            let _ = storage.remove_item(TOKEN_KEY);
            let _ = storage.remove_item(USER_KEY);
        }
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    pub fn get_token(&self) -> Option<&String> {
        self.token.as_ref()
    }

    /// The legacy Hulunote clients use `X-FUNCTOR-API-TOKEN` as the auth header.
    /// Prefer that to avoid backend/header mismatches.
    fn get_auth_token(&self) -> Option<String> {
        self.token.clone()
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<LoginResponse, String> {
        let client = reqwest::Client::new();
        let res = client
            .post(format!("{}/login/web-login", self.base_url))
            .json(&LoginRequest {
                email: email.to_string(),
                password: password.to_string(),
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            res.json().await.map_err(|e| e.to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Login failed ({status}): {body}"))
        }
    }

    fn with_auth_headers(
        mut req: reqwest::RequestBuilder,
        token: Option<String>,
    ) -> reqwest::RequestBuilder {
        if let Some(token) = token {
            // Legacy client contract.
            req = req.header("X-FUNCTOR-API-TOKEN", token.clone());
            // hulunote-rust documented contract.
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        req
    }

    async fn request_database_list(
        base_url: &str,
        token: Option<String>,
    ) -> Result<reqwest::Response, String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/get-database-list", base_url));
        let req = Self::with_auth_headers(req, token);

        req.json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| e.to_string())
    }

    fn parse_database_list_response(data: serde_json::Value) -> Vec<Database> {
        // hulunote backends have (at least) two shapes:
        // 1) { "databases": [ { id, name, description, created_at, updated_at } ] }
        // 2) { "database-list": [ { "hulunote-databases/id": ..., ... } ], "settings": {} }
        let list_value = if let Some(v) = data.get("databases") {
            v.clone()
        } else if let Some(v) = data.get("database-list") {
            v.clone()
        } else {
            serde_json::Value::Null
        };

        // Normalize null/invalid to empty list for a stable UI.
        let list = match list_value {
            serde_json::Value::Array(v) => v,
            _ => vec![],
        };

        let mut out: Vec<Database> = Vec::with_capacity(list.len());
        for item in list {
            // Preferred (new) format.
            if item.get("id").and_then(|v| v.as_str()).is_some() {
                if let Ok(db) = serde_json::from_value::<Database>(item.clone()) {
                    out.push(db);
                    continue;
                }
            }

            // Legacy/namespaced format.
            let get_s = |k: &str| item.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
            let id = get_s("hulunote-databases/id")
                .or_else(|| get_s("id"))
                .unwrap_or_default();
            let name = get_s("hulunote-databases/name")
                .or_else(|| get_s("name"))
                .unwrap_or_default();
            let description = get_s("hulunote-databases/description")
                .or_else(|| get_s("description"))
                .unwrap_or_default();
            let created_at = get_s("hulunote-databases/created-at")
                .or_else(|| get_s("created_at"))
                .unwrap_or_default();
            let updated_at = get_s("hulunote-databases/updated-at")
                .or_else(|| get_s("updated_at"))
                .unwrap_or_default();

            // Only push if it looks like a real database record.
            if !id.trim().is_empty() && !name.trim().is_empty() {
                out.push(Database {
                    id,
                    name,
                    description,
                    created_at,
                    updated_at,
                });
            }
        }

        out
    }

    fn parse_note_list_response(data: serde_json::Value) -> Vec<Note> {
        // hulunote backends have multiple shapes:
        // 1) { "notes": [ { id, database_id, title, content, created_at, updated_at } ] }
        // 2) { "note-list": [ { "hulunote-notes/id": ..., "hulunote-notes/title": ..., ... } ] }
        // 3) raw array
        let list_value = if let Some(v) = data.get("notes") {
            v.clone()
        } else if let Some(v) = data.get("note-list") {
            v.clone()
        } else {
            data.clone()
        };

        let list = match list_value {
            serde_json::Value::Array(v) => v,
            _ => vec![],
        };

        let mut out: Vec<Note> = Vec::with_capacity(list.len());
        for item in list {
            // Preferred (new) format.
            if item.get("id").and_then(|v| v.as_str()).is_some() {
                if let Ok(note) = serde_json::from_value::<Note>(item.clone()) {
                    out.push(note);
                    continue;
                }
            }

            // Legacy/namespaced format.
            let get_s = |k: &str| item.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
            let id = get_s("hulunote-notes/id")
                .or_else(|| get_s("id"))
                .unwrap_or_default();
            let database_id = get_s("hulunote-notes/database-id")
                .or_else(|| get_s("database_id"))
                .or_else(|| get_s("database-id"))
                .unwrap_or_default();
            let title = get_s("hulunote-notes/title")
                .or_else(|| get_s("title"))
                .unwrap_or_default();
            let created_at = get_s("hulunote-notes/created-at")
                .or_else(|| get_s("created_at"))
                .unwrap_or_default();
            let updated_at = get_s("hulunote-notes/updated-at")
                .or_else(|| get_s("updated_at"))
                .unwrap_or_default();

            if !id.trim().is_empty() && !database_id.trim().is_empty() {
                out.push(Note {
                    id,
                    database_id,
                    title,
                    content: String::new(),
                    created_at,
                    updated_at,
                });
            }
        }

        out
    }

    pub async fn get_all_note_list(&self, database_id: &str) -> Result<Vec<Note>, String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/get-all-note-list", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        // hulunote-rust handler expects kebab-case: `database-id`
        let res = req
            .json(&serde_json::json!({ "database-id": database_id }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            Ok(Self::parse_note_list_response(data))
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Failed to get notes ({status}): {body}"))
        }
    }

    pub async fn get_database_list(&mut self) -> Result<Vec<Database>, String> {
        // First try with current token
        let res = Self::request_database_list(&self.base_url, self.get_auth_token()).await?;

        // Backend (hulunote-rust) does not provide a refresh-token endpoint.
        // If token is invalid/expired, caller should force re-login.

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            Ok(Self::parse_database_list_response(data))
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Failed to get databases ({status}): {body}"))
        }
    }

    pub async fn create_database(
        &self,
        database_name: &str,
        description: &str,
    ) -> Result<serde_json::Value, String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/new-database", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&CreateDatabaseRequest {
                database_name: database_name.to_string(),
                description: description.to_string(),
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            res.json().await.map_err(|e| e.to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Create database failed ({status}): {body}"))
        }
    }

    pub async fn rename_database(&self, database_id: &str, new_name: &str) -> Result<(), String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/update-database", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&UpdateDatabaseRequest {
                database_id: Some(database_id.to_string()),
                id: None,
                db_name: Some(new_name.to_string()),
                is_public: None,
                is_default: None,
                is_delete: None,
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            Ok(())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Rename database failed ({status}): {body}"))
        }
    }

    pub async fn delete_database_by_id(&self, database_id: &str) -> Result<(), String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/delete-database", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&DeleteDatabaseRequest {
                database_id: Some(database_id.to_string()),
                database_name: None,
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            Ok(())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Delete database failed ({status}): {body}"))
        }
    }

    fn parse_create_note_response(data: serde_json::Value) -> Option<Note> {
        let note_value = data.get("note").cloned().unwrap_or(data);

        // Preferred (new) format.
        if note_value.get("id").and_then(|v| v.as_str()).is_some() {
            if let Ok(note) = serde_json::from_value::<Note>(note_value.clone()) {
                return Some(note);
            }
        }

        // Legacy/namespaced format.
        let get_s = |k: &str| {
            note_value
                .get(k)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };
        let id = get_s("hulunote-notes/id")
            .or_else(|| get_s("id"))
            .unwrap_or_default();
        let database_id = get_s("hulunote-notes/database-id")
            .or_else(|| get_s("database_id"))
            .or_else(|| get_s("database-id"))
            .unwrap_or_default();
        let title = get_s("hulunote-notes/title")
            .or_else(|| get_s("title"))
            .unwrap_or_default();
        let created_at = get_s("hulunote-notes/created-at")
            .or_else(|| get_s("created_at"))
            .unwrap_or_default();
        let updated_at = get_s("hulunote-notes/updated-at")
            .or_else(|| get_s("updated_at"))
            .unwrap_or_default();

        if id.trim().is_empty() {
            return None;
        }

        Some(Note {
            id,
            database_id,
            title,
            content: String::new(),
            created_at,
            updated_at,
        })
    }

    pub async fn create_note(&self, database_id: &str, title: &str) -> Result<Note, String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/new-note", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        // Be liberal: some deployments accept snake_case, some kebab-case.
        let payload = serde_json::json!({
            "database_id": database_id,
            "database-id": database_id,
            "title": title,
        });

        let res = req.json(&payload).send().await.map_err(|e| e.to_string())?;

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            Self::parse_create_note_response(data)
                .ok_or_else(|| "Create note succeeded but response could not be parsed".to_string())
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Create note failed ({status}): {body}"))
        }
    }

    pub async fn signup(
        &self,
        email: &str,
        username: &str,
        password: &str,
        registration_code: &str,
    ) -> Result<SignupResponse, String> {
        let client = reqwest::Client::new();

        // Try to be compatible with the legacy client contract used in some deployments.
        let username = if username.trim().is_empty() {
            None
        } else {
            Some(username.to_string())
        };

        let res = client
            .post(format!("{}/login/web-signup", self.base_url))
            .json(&SignupRequest {
                email: email.to_string(),
                username,
                password: password.to_string(),
                registration_code: registration_code.to_string(),
                // Leave ack/binding codes empty unless the backend requires them.
                // But provide a default binding-platform matching the legacy client.
                ack_number: None,
                binding_code: None,
                binding_platform: Some("whatsapp".to_string()),
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            res.json().await.map_err(|e| e.to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Signup failed ({status}): {body}"))
        }
    }

    pub fn logout(&mut self) {
        self.token = None;
        Self::clear_storage();
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
}

#[derive(Clone)]
pub struct AppState {
    pub api_client: RwSignal<ApiClient>,
    pub current_user: RwSignal<Option<AccountInfo>>,

    /// Loaded from backend.
    pub databases: RwSignal<Vec<Database>>,

    /// Notes for the currently selected database (Phase 5, non-paginated).
    pub notes: RwSignal<Vec<Note>>,
    pub notes_loading: RwSignal<bool>,
    pub notes_error: RwSignal<Option<String>>,

    /// Notes load guards (avoid duplicate loads + ignore stale responses).
    pub notes_request_id: RwSignal<u64>,
    pub notes_last_loaded_db_id: RwSignal<Option<String>>,

    /// Current database selection (drives routing in later phases).
    pub current_database_id: RwSignal<Option<String>>,

    /// Global UI state.
    pub sidebar_collapsed: RwSignal<bool>,

    /// Sidebar search query (Phase 3: UI + routing only).
    pub search_query: RwSignal<String>,
}

const TOKEN_KEY: &str = "hulunote_token";
const USER_KEY: &str = "hulunote_user";
const SIDEBAR_COLLAPSED_KEY: &str = "hulunote_sidebar_collapsed";
const CURRENT_DB_KEY: &str = "hulunote_current_database_id";

fn save_user_to_storage(user: &AccountInfo) {
    if let Ok(json) = serde_json::to_string(user) {
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item(USER_KEY, &json);
        }
    }
}

fn load_user_from_storage() -> Option<AccountInfo> {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        if let Ok(Some(json)) = storage.get_item(USER_KEY) {
            return serde_json::from_str(&json).ok();
        }
    }
    None
}

impl AppState {
    pub fn new() -> Self {
        let stored_client = ApiClient::load_from_storage();
        let stored_user = load_user_from_storage();

        let (sidebar_collapsed, current_database_id) = if let Some(storage) =
            web_sys::window().and_then(|w| w.local_storage().ok().flatten())
        {
            let sidebar_collapsed = storage
                .get_item(SIDEBAR_COLLAPSED_KEY)
                .ok()
                .flatten()
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);

            let current_database_id = storage.get_item(CURRENT_DB_KEY).ok().flatten();

            (sidebar_collapsed, current_database_id)
        } else {
            (false, None)
        };

        Self {
            api_client: RwSignal::new(stored_client),
            current_user: RwSignal::new(stored_user),
            databases: RwSignal::new(vec![]),
            notes: RwSignal::new(vec![]),
            notes_loading: RwSignal::new(false),
            notes_error: RwSignal::new(None),
            notes_request_id: RwSignal::new(0),
            notes_last_loaded_db_id: RwSignal::new(None),
            current_database_id: RwSignal::new(current_database_id),
            sidebar_collapsed: RwSignal::new(sidebar_collapsed),
            search_query: RwSignal::new(String::new()),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct AppContext(pub AppState);

#[component]
pub fn LoginPage() -> impl IntoView {
    let email: RwSignal<String> = RwSignal::new(String::new());
    let password: RwSignal<String> = RwSignal::new(String::new());
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let loading: RwSignal<bool> = RwSignal::new(false);

    let app_state = expect_context::<AppContext>();

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();

        let email_val = email.get();
        let password_val = password.get();
        let mut api_client = app_state.0.api_client.get_untracked();

        loading.set(true);
        error.set(None);

        spawn_local(async move {
            match api_client.login(&email_val, &password_val).await {
                Ok(response) => {
                    api_client.set_token(response.token);
                    api_client.save_to_storage();
                    save_user_to_storage(&response.hulunote);
                    app_state.0.api_client.set(api_client);
                    app_state.0.current_user.set(Some(response.hulunote));
                    let _ = window().location().set_href("/");
                }
                Err(e) => {
                    error.set(Some(e));
                }
            }
            loading.set(false);
        });
    };

    view! {
        <div class="min-h-screen bg-background">
            <div class="mx-auto flex min-h-screen w-full max-w-sm flex-col justify-center px-4 py-10">
                <div class="mb-6 flex items-center justify-center">
                    <a href="/" class="text-sm font-medium text-foreground">"Hulunote"</a>
                </div>

                <Card>
                    <CardHeader>
                        <CardTitle class="text-lg">"Log in"</CardTitle>
                        <CardDescription class="text-xs">"Use your email and password to continue."</CardDescription>
                    </CardHeader>

                    <CardContent>
                        <form class="flex flex-col gap-3" on:submit=on_submit>
                        <div class="flex flex-col gap-1.5">
                            <Label html_for="email" class="text-xs">"Email"</Label>
                            <Input
                                id="email"
                                r#type="email"
                                placeholder="you@example.com"
                                bind_value=email
                                required=true
                                class="h-8 text-sm"
                            />
                        </div>

                        <div class="flex flex-col gap-1.5">
                            <Label html_for="password" class="text-xs">"Password"</Label>
                            <Input
                                id="password"
                                r#type="password"
                                placeholder="••••••••"
                                bind_value=password
                                required=true
                                class="h-8 text-sm"
                            />
                        </div>

                        <Show when=move || error.get().is_some() fallback=|| ().into_view()>
                            {move || {
                                error.get().map(|e| {
                                    view! {
                                        <Alert class="border-destructive/30">
                                            <AlertDescription class="text-destructive text-xs">
                                                {e}
                                            </AlertDescription>
                                        </Alert>
                                    }
                                })
                            }}
                        </Show>

                        <Button
                            class="w-full"
                            size=ButtonSize::Sm
                            attr:disabled=move || loading.get()
                        >
                            <span class="inline-flex items-center gap-2">
                                <Show when=move || loading.get() fallback=|| ().into_view()>
                                    <Spinner />
                                </Show>
                                {move || if loading.get() { "Signing in..." } else { "Continue" }}
                            </span>
                        </Button>

                        <div class="pt-1 text-xs text-muted-foreground">
                            "No account? "
                            <a class="text-primary underline underline-offset-4" href="/signup">"Sign up"</a>
                        </div>
                    </form>
                    </CardContent>
                </Card>
            </div>
        </div>
    }
}

#[component]
pub fn RegistrationPage() -> impl IntoView {
    let email: RwSignal<String> = RwSignal::new(String::new());
    let username: RwSignal<String> = RwSignal::new(String::new());
    let password: RwSignal<String> = RwSignal::new(String::new());
    let confirm_password: RwSignal<String> = RwSignal::new(String::new());
    let registration_code: RwSignal<String> = RwSignal::new(String::new());
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let loading: RwSignal<bool> = RwSignal::new(false);
    let success: RwSignal<bool> = RwSignal::new(false);

    let app_state = expect_context::<AppContext>();

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();

        let email_val = email.get();
        let username_val = username.get();
        let password_val = password.get();
        let confirm_password_val = confirm_password.get();
        let reg_code_val = registration_code.get();
        let api_client = app_state.0.api_client.get_untracked();

        if password_val != confirm_password_val {
            error.set(Some("Passwords do not match".to_string()));
            return;
        }

        if password_val.len() < 6 {
            error.set(Some("Password must be at least 6 characters".to_string()));
            return;
        }

        if reg_code_val.trim().is_empty() {
            error.set(Some("Registration code is required".to_string()));
            return;
        }

        loading.set(true);
        error.set(None);

        spawn_local(async move {
            match api_client
                .signup(&email_val, &username_val, &password_val, &reg_code_val)
                .await
            {
                Ok(_response) => {
                    // Backend returns a token on signup; we keep UX simple and ask user to sign in.
                    success.set(true);
                }
                Err(e) => {
                    error.set(Some(e));
                }
            }
            loading.set(false);
        });
    };

    view! {
        <div class="min-h-screen bg-background">
            <div class="mx-auto flex min-h-screen w-full max-w-sm flex-col justify-center px-4 py-10">
                <div class="mb-6 flex items-center justify-center">
                    <a href="/" class="text-sm font-medium text-foreground">"Hulunote"</a>
                </div>

                <Card>
                    <CardHeader>
                        <CardTitle class="text-lg">"Create account"</CardTitle>
                        <CardDescription class="text-xs">"A registration code is required."</CardDescription>
                    </CardHeader>
                    <CardContent>

                    <Show
                        when=move || !success.get()
                        fallback=move || view! {
                            <Alert>
                                <AlertDescription class="text-xs">
                                    "Account created. You can now "
                                    <a class="text-primary underline underline-offset-4" href="/login">"log in"</a>
                                    "."
                                </AlertDescription>
                            </Alert>
                        }
                    >
                        <form class="flex flex-col gap-3" on:submit=on_submit>
                            <div class="flex flex-col gap-1.5">
                                <Label html_for="username" class="text-xs">"Username"</Label>
                                <Input
                                    id="username"
                                    r#type="text"
                                    placeholder="yourname"
                                    bind_value=username
                                    required=true
                                    class="h-8 text-sm"
                                />
                            </div>

                            <div class="flex flex-col gap-1.5">
                                <Label html_for="email" class="text-xs">"Email"</Label>
                                <Input
                                    id="email"
                                    r#type="email"
                                    placeholder="you@example.com"
                                    bind_value=email
                                    required=true
                                    class="h-8 text-sm"
                                />
                            </div>

                            <div class="flex flex-col gap-1.5">
                                <Label html_for="password" class="text-xs">"Password"</Label>
                                <Input
                                    id="password"
                                    r#type="password"
                                    placeholder="••••••••"
                                    bind_value=password
                                    required=true
                                    class="h-8 text-sm"
                                />
                            </div>

                            <div class="flex flex-col gap-1.5">
                                <Label html_for="confirm_password" class="text-xs">"Confirm password"</Label>
                                <Input
                                    id="confirm_password"
                                    r#type="password"
                                    placeholder="••••••••"
                                    bind_value=confirm_password
                                    required=true
                                    class="h-8 text-sm"
                                />
                            </div>

                            <div class="flex flex-col gap-1.5">
                                <Label html_for="registration_code" class="text-xs">"Registration code"</Label>
                                <Input
                                    id="registration_code"
                                    r#type="text"
                                    placeholder="FA8E-AF6E-4578-9347"
                                    bind_value=registration_code
                                    required=true
                                    class="h-8 text-sm"
                                />
                            </div>

                            <Show when=move || error.get().is_some() fallback=|| ().into_view()>
                                {move || {
                                    error.get().map(|e| {
                                        view! {
                                            <Alert class="border-destructive/30">
                                                <AlertDescription class="text-destructive text-xs">
                                                    {e}
                                                </AlertDescription>
                                            </Alert>
                                        }
                                    })
                                }}
                            </Show>

                            <Button
                                class="w-full"
                                size=ButtonSize::Sm
                                attr:disabled=move || loading.get()
                            >
                                <span class="inline-flex items-center gap-2">
                                    <Show when=move || loading.get() fallback=|| ().into_view()>
                                        <Spinner />
                                    </Show>
                                    {move || if loading.get() { "Creating..." } else { "Continue" }}
                                </span>
                            </Button>

                            <div class="pt-1 text-xs text-muted-foreground">
                                "Already have an account? "
                                <a class="text-primary underline underline-offset-4" href="/login">"Log in"</a>
                            </div>
                        </form>
                    </Show>
                    </CardContent>
                </Card>
            </div>
        </div>
    }
}

#[component]
pub fn HomePage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let current_db_id = app_state.0.current_database_id;
    let databases = app_state.0.databases;

    let current_db_name = move || {
        current_db_id
            .get()
            .and_then(|id| databases.get().into_iter().find(|d| d.id == id))
            .map(|d| d.name)
    };

    view! {
        <div class="space-y-3">
            <div class="space-y-1">
                <h1 class="text-xl font-semibold">"Hulunote"</h1>
                <p class="text-xs text-muted-foreground">
                    {move || {
                        current_db_name()
                            .map(|n| format!("Database: {}", n))
                            .unwrap_or_else(|| "Select a database in the sidebar.".to_string())
                    }}
                </p>
            </div>

            <Card>
                <CardContent>
                    <div class="text-sm text-muted-foreground">
                        "Phase 3: Layout & Navigation. Main content will become note list/editor in later phases."
                    </div>
                </CardContent>
            </Card>
        </div>
    }
}

#[component]
pub fn AppLayout(children: ChildrenFn) -> impl IntoView {
    let app_state = expect_context::<AppContext>();

    let databases = app_state.0.databases;
    let current_db_id = app_state.0.current_database_id;
    let sidebar_collapsed = app_state.0.sidebar_collapsed;

    let db_loading: RwSignal<bool> = RwSignal::new(false);
    let db_error: RwSignal<Option<String>> = RwSignal::new(None);

    // Phase 4: database create dialog state
    let create_open: RwSignal<bool> = RwSignal::new(false);
    let create_name: RwSignal<String> = RwSignal::new(String::new());
    let create_desc: RwSignal<String> = RwSignal::new(String::new());
    let create_error: RwSignal<Option<String>> = RwSignal::new(None);
    let create_loading: RwSignal<bool> = RwSignal::new(false);

    let search_query = app_state.0.search_query;
    let search_ref: NodeRef<html::Input> = NodeRef::new();

    let navigate = StoredValue::new(use_navigate());

    let sidebar_width_class = move || {
        if sidebar_collapsed.get() {
            "w-14"
        } else {
            "w-64"
        }
    };

    let persist_sidebar = move || {
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item(
                SIDEBAR_COLLAPSED_KEY,
                if sidebar_collapsed.get() { "1" } else { "0" },
            );
        }
    };

    let set_current_db = move |id: Option<String>| {
        current_db_id.set(id.clone());
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let v = id.unwrap_or_default();
            let _ = storage.set_item(CURRENT_DB_KEY, &v);
        }
    };

    let open_create_dialog = move || {
        create_name.set(String::new());
        create_desc.set(String::new());
        create_error.set(None);
        create_open.set(true);
    };

    let submit_create_database = move || {
        if create_loading.get_untracked() {
            return;
        }

        let name = create_name.get_untracked();
        if name.trim().is_empty() {
            create_error.set(Some("Database name is required".to_string()));
            return;
        }

        let desc = create_desc.get_untracked();
        let api_client = app_state.0.api_client.get_untracked();

        create_loading.set(true);
        create_error.set(None);

        spawn_local(async move {
            match api_client.create_database(&name, &desc).await {
                Ok(v) => {
                    // Try to extract the created database id from the response.
                    let new_id = v
                        .get("database")
                        .and_then(|d| {
                            d.get("hulunote-databases/id")
                                .or_else(|| d.get("id"))
                                .and_then(|x| x.as_str())
                        })
                        .map(|s| s.to_string());

                    // Refresh DB list from backend.
                    let mut c = app_state.0.api_client.get_untracked();
                    match c.get_database_list().await {
                        Ok(dbs) => {
                            app_state.0.databases.set(dbs);
                            app_state.0.api_client.set(c);
                        }
                        Err(_) => {
                            app_state.0.api_client.set(c);
                        }
                    }

                    if let Some(id) = new_id {
                        set_current_db(Some(id.clone()));
                        // Navigate to the new database home.
                        // We cannot call navigate directly here; store selection and rely on caller UI.
                        // (navigation is triggered below on the main thread)
                        navigate.with_value(|nav| {
                            nav(&format!("/db/{}", id), Default::default());
                        });
                    }

                    create_open.set(false);
                }
                Err(e) => {
                    create_error.set(Some(e));
                }
            }
            create_loading.set(false);
        });
    };

    let load_databases = move || {
        // Avoid parallel loads.
        if db_loading.get_untracked() {
            return;
        }

        let mut api_client = app_state.0.api_client.get_untracked();
        if !api_client.is_authenticated() {
            return;
        }

        db_loading.set(true);
        db_error.set(None);

        spawn_local(async move {
            match api_client.get_database_list().await {
                Ok(dbs) => {
                    app_state.0.databases.set(dbs);
                    app_state.0.api_client.set(api_client);
                }
                Err(e) => {
                    if e == "Unauthorized" {
                        api_client.logout();
                        app_state.0.api_client.set(api_client);
                        app_state.0.current_user.set(None);
                        let _ = window().location().set_href("/login");
                    } else {
                        db_error.set(Some(e));
                        app_state.0.api_client.set(api_client);
                    }
                }
            }
            db_loading.set(false);
        });
    };

    // Initial load when we enter the authenticated shell.
    Effect::new(move |_| {
        let authed = app_state.0.api_client.get().is_authenticated();
        if authed && databases.get().is_empty() {
            load_databases();
        }
    });

    // If there is no selection yet, pick a default once databases arrive.
    Effect::new(move |_| {
        let selected = current_db_id.get();
        let dbs = databases.get();
        if selected.is_none() {
            if let Some(first) = dbs.first() {
                set_current_db(Some(first.id.clone()));
            }
        }
    });

    let on_toggle_sidebar = move |_| {
        sidebar_collapsed.update(|v| *v = !*v);
        persist_sidebar();
    };

    // Keyboard shortcuts (Phase 3):
    // - Cmd/Ctrl+B: toggle sidebar
    // - Cmd/Ctrl+K: focus search
    // - Esc: blur search
    let _key_handle = window_event_listener(ev::keydown, move |ev: web_sys::KeyboardEvent| {
        let is_meta = ev.meta_key() || ev.ctrl_key();
        let key = ev.key().to_lowercase();

        // Avoid hijacking shortcuts while typing in inputs.
        let target_tag = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .map(|el| el.tag_name().to_lowercase());

        if let Some(tag) = target_tag {
            if tag == "input" || tag == "textarea" {
                // Allow Escape to still blur.
                if key != "escape" {
                    return;
                }
            }
        }

        if is_meta && key == "b" {
            ev.prevent_default();
            sidebar_collapsed.update(|v| *v = !*v);
            persist_sidebar();
            return;
        }

        if is_meta && key == "k" {
            ev.prevent_default();
            if let Some(input) = search_ref.get() {
                let _ = input.focus();
            }
            return;
        }

        if key == "escape" {
            if let Some(input) = search_ref.get() {
                let _ = input.blur();
            }
        }
    });

    let on_logout = move |_| {
        let mut api_client = app_state.0.api_client.get_untracked();
        api_client.logout();
        app_state.0.api_client.set(api_client);
        app_state.0.current_user.set(None);
        app_state.0.databases.set(vec![]);
        set_current_db(None);
        let _ = window().location().set_href("/login");
    };

    let current_db_name = move || {
        let id = current_db_id.get();
        let dbs = databases.get();
        id.and_then(|id| dbs.into_iter().find(|d| d.id == id).map(|d| d.name))
    };

    view! {
        <div class="min-h-screen bg-background text-foreground">
            <div class="mx-auto flex min-h-screen w-full max-w-5xl gap-4 px-4 py-6">
                <aside class=move || format!("{} shrink-0", sidebar_width_class())>
                    <div class="sticky top-6 space-y-4">
                        <div class="flex items-center justify-between">
                            <a href="/" class="text-sm font-medium text-foreground">
                                <Show when=move || !sidebar_collapsed.get() fallback=|| view! { "H" }>
                                    "Hulunote"
                                </Show>
                            </a>

                            <Button
                                variant=ButtonVariant::Outline
                                size=ButtonSize::Icon
                                on:click=on_toggle_sidebar
                                attr:title="Toggle sidebar"
                                class="h-8 w-8"
                            >
                                <span class="text-xs text-muted-foreground">
                                    {move || if sidebar_collapsed.get() { ">" } else { "<" }}
                                </span>
                            </Button>
                        </div>

                        <Show
                            when=move || !sidebar_collapsed.get()
                            fallback=|| view! {
                                <Card>
                                    <CardContent>
                                        <div class="text-xs text-muted-foreground">"Sidebar collapsed"</div>
                                    </CardContent>
                                </Card>
                            }
                        >
                            <Card>
                                <CardContent class="p-3">
                                    <div class="flex items-center gap-2">
                                        <span class="sr-only">"Search"</span>
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            width="16"
                                            height="16"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            class="shrink-0 text-muted-foreground"
                                            aria-hidden="true"
                                        >
                                            <circle cx="11" cy="11" r="8"></circle>
                                            <path d="m21 21-4.3-4.3"></path>
                                        </svg>

                                        <div class="min-w-0 flex-1">
                                            <Input
                                                node_ref=search_ref
                                                r#type="search"
                                                placeholder="Search…"
                                                bind_value=search_query
                                                class="h-8 text-sm"
                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                    if ev.key() == "Enter" {
                                                        let q = search_query.get();
                                                        navigate.with_value(|nav| {
                                                            nav(&format!("/search?q={}", urlencoding::encode(&q)), Default::default());
                                                        });
                                                    }
                                                }
                                            />
                                        </div>

                                        <div class="hidden shrink-0 items-center gap-1 text-xs text-muted-foreground sm:flex">
                                            <span class="rounded-md border border-border bg-surface px-2 py-1 font-mono text-[11px]">
                                                "⌘K"
                                            </span>
                                        </div>
                                    </div>
                                </CardContent>
                            </Card>

                            <Card>
                                <CardHeader class="flex flex-row items-center justify-end p-3">
                                    <span class="sr-only">"Databases"</span>
                                    <div class="flex items-center gap-2">
                                        <Button
                                            variant=ButtonVariant::Ghost
                                            size=ButtonSize::Icon
                                            on:click=move |_| open_create_dialog()
                                            attr:title="New database"
                                            class="h-7 w-7"
                                        >
                                            <span class="text-xs text-muted-foreground">"+"</span>
                                        </Button>
                                        <Button
                                            variant=ButtonVariant::Ghost
                                            size=ButtonSize::Icon
                                            on:click=move |_| load_databases()
                                            attr:title="Refresh"
                                            class="h-7 w-7"
                                        >
                                            <span class="text-xs text-muted-foreground">"↻"</span>
                                        </Button>
                                        </div>
                                </CardHeader>
                                <CardContent class="p-3 pt-0">
                                    <Show when=move || db_error.get().is_some() fallback=|| ().into_view()>
                                    {move || db_error.get().map(|e| view! {
                                        <div class="mt-2 text-[11px] text-destructive">{e}</div>
                                    })}
                                </Show>

                                <div class="mt-2 space-y-1">
                                    <Show
                                        when=move || !databases.get().is_empty()
                                        fallback=move || view! {
                                            <div class="text-[11px] text-muted-foreground">
                                                {move || if db_loading.get() { "Loading..." } else { "No databases" }}
                                            </div>
                                        }
                                    >
                                        {move || {
                                            let selected = current_db_id.get();
                                            databases
                                                .get()
                                                .into_iter()
                                                .map(|db| {
                                                    let is_selected = selected.as_deref() == Some(db.id.as_str());
                                                    let variant = if is_selected {
                                                        ButtonVariant::Accent
                                                    } else {
                                                        ButtonVariant::Ghost
                                                    };

                                                    let id = db.id.clone();
                                                    view! {
                                                        <Button
                                                            variant=variant
                                                            size=ButtonSize::Sm
                                                            class="w-full justify-start"
                                                            attr:aria-current=move || if is_selected { Some("page") } else { None }
                                                            href=format!("/db/{}", id)
                                                        >
                                                            {db.name}
                                                        </Button>
                                                    }
                                                })
                                                .collect_view()
                                        }}
                                    </Show>
                                </div>
                                </CardContent>
                            </Card>

                            <Card>
                                <CardContent class="p-3">
                                    <span class="sr-only">"Navigation"</span>
                                    <div class="space-y-1">
                                        <Button
                                        variant=ButtonVariant::Ghost
                                        size=ButtonSize::Sm
                                        class="w-full justify-start"
                                        on:click=move |_| {
                                            navigate.with_value(|nav| nav("/settings", Default::default()));
                                        }
                                    >
                                        "Settings"
                                    </Button>
                                    </div>
                                </CardContent>
                            </Card>

                            <Card>
                                <CardContent class="p-3">
                                    <span class="sr-only">"Account"</span>
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=ButtonSize::Sm
                                        on:click=on_logout
                                        class="w-full"
                                    >
                                        "Sign out"
                                    </Button>
                                </CardContent>
                            </Card>
                        </Show>
                    </div>
                </aside>

                <main class="min-w-0 flex-1">
                    <div class="mb-4 flex items-center justify-between">
                        <div class="space-y-0.5">
                            <div class="text-sm font-medium">
                                {move || {
                                    current_db_name()
                                        .map(|n| n.to_string())
                                        .unwrap_or_else(|| "Hulunote".to_string())
                                }}
                            </div>
                        </div>
                        <div class="flex items-center gap-2">
                            <Button
                                variant=ButtonVariant::Outline
                                size=ButtonSize::Sm
                                on:click=move |_| {
                                    if let Some(id) = current_db_id.get() {
                                        navigate.with_value(|nav| {
                                            nav(&format!("/db/{}", id), Default::default());
                                        });
                                    } else {
                                        navigate.with_value(|nav| nav("/", Default::default()));
                                    }
                                }
                            >
                                "Current DB"
                            </Button>
                        </div>
                    </div>
                    {children()}
                </main>

                <Show when=move || create_open.get() fallback=|| ().into_view()>
                    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 px-4">
                        <div class="w-full max-w-sm rounded-md border border-border bg-background p-4 shadow-lg">
                            <div class="mb-3 space-y-1">
                                <div class="text-sm font-medium">"New database"</div>
                                <div class="text-xs text-muted-foreground">
                                    "Create a new database (max 5)."
                                </div>
                            </div>

                            <div class="space-y-2">
                                <div class="space-y-1">
                                    <Label class="text-xs">"Name"</Label>
                                    <Input bind_value=create_name class="h-8 text-sm" placeholder="My Notebook" />
                                </div>
                                <div class="space-y-1">
                                    <Label class="text-xs">"Description"</Label>
                                    <Input bind_value=create_desc class="h-8 text-sm" placeholder="Optional" />
                                </div>

                                <Show when=move || create_error.get().is_some() fallback=|| ().into_view()>
                                    {move || create_error.get().map(|e| view! {
                                        <Alert class="border-destructive/30">
                                            <AlertDescription class="text-destructive text-xs">{e}</AlertDescription>
                                        </Alert>
                                    })}
                                </Show>

                                <div class="flex items-center justify-end gap-2 pt-2">
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=ButtonSize::Sm
                                        attr:disabled=move || create_loading.get()
                                        on:click=move |_| create_open.set(false)
                                    >
                                        "Cancel"
                                    </Button>
                                    <Button
                                        size=ButtonSize::Sm
                                        attr:disabled=move || create_loading.get()
                                        on:click=move |_| submit_create_database()
                                    >
                                        <span class="inline-flex items-center gap-2">
                                            <Show when=move || create_loading.get() fallback=|| ().into_view()>
                                                <Spinner />
                                            </Show>
                                            {move || if create_loading.get() { "Creating..." } else { "Create" }}
                                        </span>
                                    </Button>
                                </div>
                            </div>
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}

#[component]
pub fn RootAuthed(children: ChildrenFn) -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let is_authenticated = move || app_state.0.api_client.get().is_authenticated();

    // Store children so the view macro sees an `Fn` (not an `FnOnce`).
    let children = StoredValue::new(children);

    view! {
        <Show when=is_authenticated fallback=move || view! { <LoginPage /> }>
            <AppLayout>
                {move || children.with_value(|c| c())}
            </AppLayout>
        </Show>
    }
}

#[component]
pub fn RootPage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let is_authenticated = move || app_state.0.api_client.get().is_authenticated();
    let navigate = use_navigate();

    // If we already have a database selected, treat `/` as a redirect to `/db/:db_id`.
    Effect::new(move |_| {
        if is_authenticated() {
            if let Some(id) = app_state.0.current_database_id.get() {
                if !id.trim().is_empty() {
                    navigate(&format!("/db/{}", id), Default::default());
                }
            }
        }
    });

    view! {
        <RootAuthed>
            <HomePage />
        </RootAuthed>
    }
}

#[derive(Params, PartialEq, Clone, Debug)]
pub struct DbRouteParams {
    pub db_id: Option<String>,
}

#[derive(Params, PartialEq, Clone, Debug)]
pub struct NoteRouteParams {
    pub db_id: Option<String>,
    pub note_id: Option<String>,
}

#[component]
pub fn NotePage() -> impl IntoView {
    let params = leptos_router::hooks::use_params::<NoteRouteParams>();

    // Use closures so params access happens inside a reactive tracking context.
    let db_id = move || params.get().ok().and_then(|p| p.db_id).unwrap_or_default();

    let note_id = move || {
        params
            .get()
            .ok()
            .and_then(|p| p.note_id)
            .unwrap_or_default()
    };

    view! {
        <div class="space-y-3">
            <div class="space-y-1">
                <h1 class="text-xl font-semibold">"Note"</h1>
                <p class="text-xs text-muted-foreground">{move || format!("db_id: {}", db_id())}</p>
                <p class="text-xs text-muted-foreground">{move || format!("note_id: {}", note_id())}</p>
            </div>
            <Card>
                <CardContent>
                    <div class="text-sm text-muted-foreground">
                        "Note detail/editor will be implemented next (Phase 5/6)."
                    </div>
                </CardContent>
            </Card>
        </div>
    }
}

#[component]
pub fn DbHomePage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let params = leptos_router::hooks::use_params::<DbRouteParams>();
    let navigate = StoredValue::new(use_navigate());

    let rename_open: RwSignal<bool> = RwSignal::new(false);

    // Phase 5: create note (non-paginated)
    let create_note_loading: RwSignal<bool> = RwSignal::new(false);
    let create_note_error: RwSignal<Option<String>> = RwSignal::new(None);
    let rename_value: RwSignal<String> = RwSignal::new(String::new());
    let rename_loading: RwSignal<bool> = RwSignal::new(false);
    let rename_error: RwSignal<Option<String>> = RwSignal::new(None);

    let delete_open: RwSignal<bool> = RwSignal::new(false);
    let delete_confirm: RwSignal<String> = RwSignal::new(String::new());
    let delete_loading: RwSignal<bool> = RwSignal::new(false);
    let delete_error: RwSignal<Option<String>> = RwSignal::new(None);

    // Params are reactive; read tracked in effects/views, and read untracked in event handlers.
    let db_id = move || params.get().ok().and_then(|p| p.db_id).unwrap_or_default();
    let db_id_untracked = move || {
        params
            .get_untracked()
            .ok()
            .and_then(|p| p.db_id)
            .unwrap_or_default()
    };

    let persist_current_db = move |id: &str| {
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item(CURRENT_DB_KEY, id);
        }
    };

    // Notes loading guards (avoid duplicate loads + ignore stale responses).
    // Store guard state on AppState so it survives route changes.
    let load_notes_for_sv = StoredValue::new(move |id: String, force: bool| {
        if id.trim().is_empty() {
            return;
        }

        if !force {
            let already_loaded = app_state
                .0
                .notes_last_loaded_db_id
                .get_untracked()
                .as_deref()
                == Some(id.as_str());
            let has_error = app_state.0.notes_error.get_untracked().is_some();
            let is_loading = app_state.0.notes_loading.get_untracked();

            if already_loaded && !has_error && !is_loading {
                return;
            }
        }

        app_state.0.notes_last_loaded_db_id.set(Some(id.clone()));

        let req_id = app_state
            .0
            .notes_request_id
            .get_untracked()
            .saturating_add(1);
        app_state.0.notes_request_id.set(req_id);

        app_state.0.notes_loading.set(true);
        app_state.0.notes_error.set(None);

        let api_client = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            let result = api_client.get_all_note_list(&id).await;

            // Ignore stale responses.
            if app_state.0.notes_request_id.get_untracked() != req_id {
                return;
            }

            match result {
                Ok(notes) => {
                    app_state.0.notes.set(notes);
                }
                Err(e) => {
                    if e == "Unauthorized" {
                        let mut c = app_state.0.api_client.get_untracked();
                        c.logout();
                        app_state.0.api_client.set(c);
                        app_state.0.current_user.set(None);
                        let _ = window().location().set_href("/login");
                    } else {
                        app_state.0.notes_error.set(Some(e));
                        app_state.0.notes.set(vec![]);
                    }
                }
            }
            app_state.0.notes_loading.set(false);
        });
    });

    // Keep global selection in sync with URL.
    Effect::new(move |_| {
        let id = db_id();
        if !id.trim().is_empty() && app_state.0.current_database_id.get() != Some(id.clone()) {
            app_state.0.current_database_id.set(Some(id.clone()));
            persist_current_db(&id);
        }
    });

    // Phase 5 (non-paginated): load notes for current database.
    Effect::new(move |_| {
        load_notes_for_sv.with_value(|f| {
            f(db_id(), false);
        });
    });

    let db = move || {
        let id = db_id();
        app_state.0.databases.get().into_iter().find(|d| d.id == id)
    };

    let refresh_databases = move || {
        let mut c = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            match c.get_database_list().await {
                Ok(dbs) => {
                    app_state.0.databases.set(dbs);
                }
                Err(e) => {
                    if e == "Unauthorized" {
                        c.logout();
                        app_state.0.api_client.set(c);
                        app_state.0.current_user.set(None);
                        let _ = window().location().set_href("/login");
                        return;
                    }
                }
            }
            app_state.0.api_client.set(c);
        });
    };

    let _refresh_databases = move || {
        let mut c = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            if let Ok(dbs) = c.get_database_list().await {
                app_state.0.databases.set(dbs);
            }
            app_state.0.api_client.set(c);
        });
    };

    let on_open_rename = move |_| {
        rename_error.set(None);
        if let Some(d) = db() {
            rename_value.set(d.name);
        }
        rename_open.set(true);
    };

    let on_submit_rename = move |_| {
        if rename_loading.get_untracked() {
            return;
        }
        let id = db_id();
        let new_name = rename_value.get_untracked();
        if new_name.trim().is_empty() {
            rename_error.set(Some("Name cannot be empty".to_string()));
            return;
        }
        let api_client = app_state.0.api_client.get_untracked();

        rename_loading.set(true);
        rename_error.set(None);

        spawn_local(async move {
            match api_client.rename_database(&id, &new_name).await {
                Ok(_) => {
                    refresh_databases();
                    rename_open.set(false);
                }
                Err(e) => rename_error.set(Some(e)),
            }
            rename_loading.set(false);
        });
    };

    let on_open_delete = move |_| {
        delete_confirm.set(String::new());
        delete_error.set(None);
        delete_open.set(true);
    };

    let on_submit_delete = move |_| {
        if delete_loading.get_untracked() {
            return;
        }

        let id = db_id();
        let name = db().map(|d| d.name).unwrap_or_default();
        let confirm = delete_confirm.get_untracked();
        if confirm.trim() != name.trim() {
            delete_error.set(Some(
                "Type the database name to confirm deletion".to_string(),
            ));
            return;
        }

        let api_client = app_state.0.api_client.get_untracked();
        delete_loading.set(true);
        delete_error.set(None);

        spawn_local(async move {
            match api_client.delete_database_by_id(&id).await {
                Ok(_) => {
                    // Reload DBs and navigate to the first remaining DB (or /).
                    let mut c = app_state.0.api_client.get_untracked();
                    if let Ok(dbs) = c.get_database_list().await {
                        app_state.0.databases.set(dbs.clone());
                        if let Some(first) = dbs.first() {
                            app_state.0.current_database_id.set(Some(first.id.clone()));
                            persist_current_db(&first.id);
                            navigate.with_value(|nav| {
                                nav(&format!("/db/{}", first.id), Default::default());
                            });
                        } else {
                            app_state.0.current_database_id.set(None);
                            persist_current_db("");
                            navigate.with_value(|nav| {
                                nav("/", Default::default());
                            });
                        }
                    }
                    app_state.0.api_client.set(c);
                    delete_open.set(false);
                }
                Err(e) => delete_error.set(Some(e)),
            }
            delete_loading.set(false);
        });
    };

    view! {
        <div class="space-y-3">
            <div class="flex items-start justify-between gap-3">
                <div class="space-y-1">
                    <h1 class="text-xl font-semibold">
                        {move || db().map(|d| d.name).unwrap_or_else(|| "Database".to_string())}
                    </h1>
                    <p class="text-xs text-muted-foreground">{move || format!("db_id: {}", db_id())}</p>
                </div>

                <div class="flex items-center gap-2">
                    <Button
                        variant=ButtonVariant::Outline
                        size=ButtonSize::Sm
                        class="bg-surface hover:bg-accent-soft"
                        on:click=on_open_rename
                    >
                        "Rename"
                    </Button>
                    <Button
                        variant=ButtonVariant::Outline
                        size=ButtonSize::Sm
                        class="border-destructive/40 text-destructive hover:bg-surface-hover"
                        on:click=on_open_delete
                    >
                        "Delete"
                    </Button>
                </div>
            </div>

            <Card>
                <CardContent>
                    <div class="flex items-center justify-between gap-3">
                        <div class="text-sm font-medium">"Notes"</div>
                        <Button
                            variant=ButtonVariant::Outline
                            size=ButtonSize::Sm
                            attr:disabled=move || create_note_loading.get()
                            on:click=move |_| {
                                if create_note_loading.get_untracked() {
                                    return;
                                }

                                create_note_loading.set(true);
                                create_note_error.set(None);

                                let id = db_id_untracked();
                                let title = next_available_daily_note_title(&app_state.0.notes.get_untracked());
                                let api_client = app_state.0.api_client.get_untracked();
                                let load_notes_for_sv = load_notes_for_sv;

                                spawn_local(async move {
                                    match api_client.create_note(&id, &title).await {
                                        Ok(note) => {
                                            // Refresh list then navigate to note.
                                            load_notes_for_sv.with_value(|f| {
                                                f(id.clone(), true);
                                            });
                                            navigate.with_value(|nav| {
                                                nav(
                                                    &format!("/db/{}/note/{}", id, note.id),
                                                    Default::default(),
                                                );
                                            });
                                        }
                                        Err(e) => {
                                            if e == "Unauthorized" {
                                                let mut c = app_state.0.api_client.get_untracked();
                                                c.logout();
                                                app_state.0.api_client.set(c);
                                                app_state.0.current_user.set(None);
                                                let _ = window().location().set_href("/login");
                                            } else {
                                                create_note_error.set(Some(e));
                                            }
                                        }
                                    }
                                    create_note_loading.set(false);
                                });
                            }
                            attr:title="New note"
                        >
                            {move || if create_note_loading.get() { "Creating..." } else { "New" }}
                        </Button>
                    </div>

                    <div class="mt-3 space-y-2">
                        <Show when=move || create_note_error.get().is_some() fallback=|| ().into_view()>
                            {move || {
                                create_note_error.get().map(|e| {
                                    view! {
                                        <Alert class="border-destructive/30">
                                            <AlertDescription class="text-destructive text-xs">{e}</AlertDescription>
                                        </Alert>
                                    }
                                })
                            }}
                        </Show>

                        <Show
                            when=move || !app_state.0.notes_loading.get()
                            fallback=move || view! {
                                <div class="flex items-center gap-2 text-sm text-muted-foreground">
                                    <Spinner />
                                    "Loading notes…"
                                </div>
                            }
                        >
                            <Show
                                when=move || app_state.0.notes_error.get().is_none()
                                fallback=move || view! {
                                    <Alert class="border-destructive/30">
                                        <AlertDescription class="text-destructive text-xs">
                                            {move || app_state.0.notes_error.get().unwrap_or_default()}
                                        </AlertDescription>
                                    </Alert>
                                }
                            >
                                <Show
                                    when=move || !app_state.0.notes.get().is_empty()
                                    fallback=move || view! {
                                        <div class="text-sm text-muted-foreground">"No notes yet."</div>
                                    }
                                >
                                    <div class="space-y-1">
                                        {move || {
                                            app_state
                                                .0
                                                .notes
                                                .get()
                                                .into_iter()
                                                .map(|n| {
                                                    view! {
                                                        <a
                                                            href=format!("/db/{}/note/{}", db_id(), n.id)
                                                            class="block rounded-md border border-border bg-background px-3 py-2 transition-colors hover:bg-surface-hover"
                                                        >
                                                            <div class="min-w-0">
                                                                <div class="truncate text-sm font-medium">{n.title}</div>
                                                                <div class="truncate text-xs text-muted-foreground">{n.updated_at}</div>
                                                            </div>
                                                        </a>
                                                    }
                                                })
                                                .collect_view()
                                        }}
                                    </div>
                                </Show>
                            </Show>
                        </Show>
                    </div>
                </CardContent>
            </Card>

            <Show when=move || rename_open.get() fallback=|| ().into_view()>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 px-4">
                    <div class="w-full max-w-sm rounded-md border border-border bg-background p-4 shadow-lg">
                        <div class="mb-3 space-y-1">
                            <div class="text-sm font-medium">"Rename database"</div>
                            <div class="text-xs text-muted-foreground">"Only the name can be updated (backend limitation)."</div>
                        </div>

                        <div class="space-y-2">
                            <div class="space-y-1">
                                <Label class="text-xs">"New name"</Label>
                                <Input bind_value=rename_value class="h-8 text-sm" />
                            </div>

                            <Show when=move || rename_error.get().is_some() fallback=|| ().into_view()>
                                {move || rename_error.get().map(|e| view! {
                                    <Alert class="border-destructive/30">
                                        <AlertDescription class="text-destructive text-xs">{e}</AlertDescription>
                                    </Alert>
                                })}
                            </Show>

                            <div class="flex items-center justify-end gap-2 pt-2">
                                <Button
                                    variant=ButtonVariant::Outline
                                    size=ButtonSize::Sm
                                    attr:disabled=move || rename_loading.get()
                                    on:click=move |_| rename_open.set(false)
                                >
                                    "Cancel"
                                </Button>
                                <Button
                                    size=ButtonSize::Sm
                                    attr:disabled=move || rename_loading.get()
                                    on:click=on_submit_rename
                                >
                                    <span class="inline-flex items-center gap-2">
                                        <Show when=move || rename_loading.get() fallback=|| ().into_view()>
                                            <Spinner />
                                        </Show>
                                        {move || if rename_loading.get() { "Saving..." } else { "Save" }}
                                    </span>
                                </Button>
                            </div>
                        </div>
                    </div>
                </div>
            </Show>

            <Show when=move || delete_open.get() fallback=|| ().into_view()>
                <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30 px-4">
                    <div class="w-full max-w-sm rounded-md border border-border bg-background p-4 shadow-lg">
                        <div class="mb-3 space-y-1">
                            <div class="text-sm font-medium">"Delete database"</div>
                            <div class="text-xs text-muted-foreground">
                                {move || {
                                    let name = db().map(|d| d.name).unwrap_or_default();
                                    format!("Type '{}' to confirm.", name)
                                }}
                            </div>
                        </div>

                        <div class="space-y-2">
                            <Input bind_value=delete_confirm class="h-8 text-sm" />

                            <Show when=move || delete_error.get().is_some() fallback=|| ().into_view()>
                                {move || delete_error.get().map(|e| view! {
                                    <Alert class="border-destructive/30">
                                        <AlertDescription class="text-destructive text-xs">{e}</AlertDescription>
                                    </Alert>
                                })}
                            </Show>

                            <div class="flex items-center justify-end gap-2 pt-2">
                                <Button
                                    variant=ButtonVariant::Outline
                                    size=ButtonSize::Sm
                                    attr:disabled=move || delete_loading.get()
                                    on:click=move |_| delete_open.set(false)
                                >
                                    "Cancel"
                                </Button>
                                <Button
                                    size=ButtonSize::Sm
                                    attr:disabled=move || delete_loading.get()
                                    on:click=on_submit_delete
                                >
                                    <span class="inline-flex items-center gap-2">
                                        <Show when=move || delete_loading.get() fallback=|| ().into_view()>
                                            <Spinner />
                                        </Show>
                                        {move || if delete_loading.get() { "Deleting..." } else { "Delete" }}
                                    </span>
                                </Button>
                            </div>
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[component]
pub fn SearchPage() -> impl IntoView {
    let query = use_query_map();
    let q = move || query.get().get("q").unwrap_or_default();

    view! {
        <div class="space-y-3">
            <div class="space-y-1">
                <h1 class="text-xl font-semibold">"Search"</h1>
                <p class="text-xs text-muted-foreground">{move || format!("q = {}", q())}</p>
            </div>
            <div class="rounded-md border border-border bg-muted p-4 text-sm text-muted-foreground">
                "Phase 3: Search UI is scaffolded. Results will be implemented in Phase 10."
            </div>
        </div>
    }
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    view! {
        <div class="space-y-3">
            <div class="space-y-1">
                <h1 class="text-xl font-semibold">"Settings"</h1>
                <p class="text-xs text-muted-foreground">"Phase 3 placeholder"</p>
            </div>
            <div class="rounded-md border border-border bg-muted p-4 text-sm text-muted-foreground">
                "Appearance/editor/account settings will be implemented in later phases."
            </div>
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_context(AppContext(AppState::new()));

    // IMPORTANT:
    // - Leptos CSR requires the `csr` feature on `leptos`.
    // - router hooks require a <Router> context.
    view! {
        <Router>
            <Routes fallback=|| view! { <div class="px-4 py-8 text-xs text-muted-foreground">"Not found"</div> }>
                <Route path=path!("login") view=LoginPage />
                <Route path=path!("signup") view=RegistrationPage />
                <Route path=path!("db/:db_id") view=move || view! {
                    <RootAuthed>
                        <DbHomePage />
                    </RootAuthed>
                } />
                <Route path=path!("db/:db_id/note/:note_id") view=move || view! {
                    <RootAuthed>
                        <NotePage />
                    </RootAuthed>
                } />
                <Route path=path!("search") view=move || view! {
                    <RootAuthed>
                        <SearchPage />
                    </RootAuthed>
                } />
                <Route path=path!("settings") view=move || view! {
                    <RootAuthed>
                        <SettingsPage />
                    </RootAuthed>
                } />
                <Route path=path!("") view=RootPage />
            </Routes>
        </Router>
    }
}

// WASM-only tests (run with `cargo test --target wasm32-unknown-unknown` + wasm-bindgen-test-runner)
#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_api_client_storage_roundtrip_token() {
        ApiClient::clear_storage();

        let mut c = ApiClient::load_from_storage();
        assert!(!c.is_authenticated());

        c.set_token("t1".to_string());
        c.save_to_storage();

        let c2 = ApiClient::load_from_storage();
        assert_eq!(c2.get_token().map(|s| s.as_str()), Some("t1"));

        ApiClient::clear_storage();
        let c3 = ApiClient::load_from_storage();
        assert!(c3.get_token().is_none());
    }

    #[wasm_bindgen_test]
    fn test_user_storage_roundtrip() {
        let user = AccountInfo {
            extra: serde_json::json!({"id": 1, "username": "u"}),
        };
        save_user_to_storage(&user);
        let loaded = load_user_from_storage().expect("should load user from localStorage");
        assert_eq!(loaded.extra["username"], "u");
    }
}

// Only register the WASM start function for normal builds (not for tests),
// otherwise wasm-bindgen-test will end up with multiple entry symbols.
#[cfg_attr(all(target_arch = "wasm32", not(test)), wasm_bindgen(start))]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_response_contract_deserialize() {
        // Contract based on hulunote-rust: handlers/auth.rs
        let json = r#"{
            "token": "jwt-token",
            "hulunote": {"id": 1, "username": "u", "mail": "u@example.com"},
            "region": null
        }"#;
        let parsed: LoginResponse =
            serde_json::from_str(json).expect("login response should parse");
        assert_eq!(parsed.token, "jwt-token");
        // hulunote is opaque; just ensure it's an object
        assert!(parsed.hulunote.extra.is_object());
        assert!(parsed.region.is_none());
    }

    #[test]
    fn test_signup_response_contract_deserialize() {
        // Contract based on hulunote-rust: handlers/auth.rs
        let json = r#"{
            "token": "jwt-token",
            "hulunote": {"id": 1, "username": "u"},
            "database": "u-1234",
            "region": null
        }"#;
        let parsed: SignupResponse =
            serde_json::from_str(json).expect("signup response should parse");
        assert_eq!(parsed.token, "jwt-token");
        assert_eq!(parsed.database.as_deref(), Some("u-1234"));
        assert!(parsed.hulunote.extra.is_object());
    }

    #[test]
    fn test_signup_request_serialization_includes_registration_code() {
        let req = SignupRequest {
            email: "u@example.com".to_string(),
            username: None,
            password: "pass".to_string(),
            registration_code: "FA8E-AF6E-4578-9347".to_string(),
            ack_number: None,
            binding_code: None,
            binding_platform: Some("whatsapp".to_string()),
        };
        let v = serde_json::to_value(req).expect("should serialize");
        assert_eq!(v["email"], "u@example.com");
        assert_eq!(v["registration_code"], "FA8E-AF6E-4578-9347");
        assert_eq!(v["binding-platform"], "whatsapp");
    }

    #[test]
    fn test_api_client_new() {
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert_eq!(client.base_url, "http://localhost:6689");
        assert!(client.token.is_none());
    }

    #[test]
    fn test_api_client_set_token() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_token("test-token".to_string());
        assert_eq!(client.token, Some("test-token".to_string()));
    }

    #[test]
    fn test_api_client_get_auth_token_without_token() {
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert!(client.get_auth_token().is_none());
    }

    #[test]
    fn test_api_client_get_auth_token_with_token() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_token("my-jwt-token".to_string());
        let token = client.get_auth_token().expect("Should have auth token");
        assert_eq!(token, "my-jwt-token");
    }

    #[test]
    fn test_api_client_no_refresh_token_support() {
        // hulunote-rust does not expose refresh tokens.
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert!(client.get_token().is_none());
    }

    #[test]
    fn test_api_client_is_authenticated_false() {
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert!(!client.is_authenticated());
    }

    #[test]
    fn test_api_client_is_authenticated_true() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_token("my-jwt-token".to_string());
        assert!(client.is_authenticated());
    }

    #[test]
    fn test_parse_database_list_response_new_shape() {
        let v = serde_json::json!({
            "databases": [
                {
                    "id": "db1",
                    "name": "My DB",
                    "description": "desc",
                    "created_at": "t1",
                    "updated_at": "t2"
                }
            ]
        });

        let out = ApiClient::parse_database_list_response(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "db1");
        assert_eq!(out[0].name, "My DB");
    }

    #[test]
    fn test_parse_database_list_response_legacy_shape() {
        let v = serde_json::json!({
            "database-list": [
                {
                    "hulunote-databases/id": "0a1dd8e1-e255-4b35-937e-bac27dea1274",
                    "hulunote-databases/name": "ypyf-9361",
                    "hulunote-databases/description": "",
                    "hulunote-databases/created-at": "2026-02-08T15:59:24.130460+00:00",
                    "hulunote-databases/updated-at": "2026-02-08T15:59:24.130460+00:00"
                }
            ],
            "settings": {}
        });

        let out = ApiClient::parse_database_list_response(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "ypyf-9361");
        assert!(out[0].id.starts_with("0a1dd8e1"));
    }

    #[test]
    fn test_parse_note_list_response_expected_shape() {
        let v = serde_json::json!({
            "notes": [
                {
                    "id": "n1",
                    "database_id": "db1",
                    "title": "Hello",
                    "content": "",
                    "created_at": "t1",
                    "updated_at": "t2"
                }
            ]
        });

        let out = ApiClient::parse_note_list_response(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "n1");
        assert_eq!(out[0].database_id, "db1");
        assert_eq!(out[0].title, "Hello");
    }

    #[test]
    fn test_parse_note_list_response_legacy_shape_note_list() {
        let v = serde_json::json!({
            "note-list": [
                {
                    "hulunote-notes/id": "n2",
                    "hulunote-notes/database-id": "db2",
                    "hulunote-notes/title": "Legacy",
                    "hulunote-notes/created-at": "t1",
                    "hulunote-notes/updated-at": "t2"
                }
            ]
        });

        let out = ApiClient::parse_note_list_response(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "n2");
        assert_eq!(out[0].database_id, "db2");
        assert_eq!(out[0].title, "Legacy");
        assert_eq!(out[0].updated_at, "t2");
    }

    #[test]
    fn test_next_available_daily_note_title_adds_suffix() {
        let base = "20260209";

        let notes = vec![
            Note {
                id: "n1".to_string(),
                database_id: "db".to_string(),
                title: base.to_string(),
                content: "".to_string(),
                created_at: "t1".to_string(),
                updated_at: "t2".to_string(),
            },
            Note {
                id: "n2".to_string(),
                database_id: "db".to_string(),
                title: format!("{}-2", base),
                content: "".to_string(),
                created_at: "t1".to_string(),
                updated_at: "t2".to_string(),
            },
        ];

        let next = next_available_daily_note_title_for_date(base, &notes);
        assert_eq!(next, format!("{}-3", base));
    }
}
