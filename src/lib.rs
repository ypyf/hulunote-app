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
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::params::Params;
use leptos_router::path;
use serde::{Deserialize, Serialize};
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

    #[serde(rename = "note-id")]
    pub note_id: String,

    /// Parent nav id. Root uses all-zero UUID.
    pub parid: String,

    #[serde(rename = "same-deep-order")]
    pub same_deep_order: f32,

    pub content: String,

    #[serde(rename = "is-display")]
    pub is_display: bool,

    #[serde(rename = "is-delete")]
    pub is_delete: bool,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GetNoteNavsRequest {
    #[serde(rename = "note-id")]
    pub note_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreateOrUpdateNavRequest {
    #[serde(rename = "note-id")]
    pub note_id: String,

    /// Nav id (omit to create).
    pub id: Option<String>,

    /// Parent nav id.
    pub parid: Option<String>,

    pub content: Option<String>,

    /// Sort key within siblings (midpoint order).
    pub order: Option<f32>,

    #[serde(rename = "is-display")]
    pub is_display: Option<bool>,

    #[serde(rename = "is-delete")]
    pub is_delete: Option<bool>,

    pub properties: Option<String>,
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
    pub username: String,
    pub password: String,

    /// Registration/invite code.
    #[serde(rename = "registration-code")]
    pub registration_code: String,
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
        // Canonical contract: `get-database-list` returns `database-list` with namespaced keys.
        let list = data
            .get("database-list")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut out: Vec<Database> = Vec::with_capacity(list.len());
        for item in list {
            let get_s = |k: &str| item.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());

            let id = get_s("hulunote-databases/id").unwrap_or_default();
            let name = get_s("hulunote-databases/name").unwrap_or_default();
            let description = get_s("hulunote-databases/description").unwrap_or_default();
            let created_at = get_s("hulunote-databases/created-at").unwrap_or_default();
            let updated_at = get_s("hulunote-databases/updated-at").unwrap_or_default();

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
        // Canonical contract: `get-all-note-list` returns `note-list` with namespaced keys.
        let list = data
            .get("note-list")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut out: Vec<Note> = Vec::with_capacity(list.len());
        for item in list {
            let get_s = |k: &str| item.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());

            let id = get_s("hulunote-notes/id").unwrap_or_default();
            let database_id = get_s("hulunote-notes/database-id").unwrap_or_default();
            let title = get_s("hulunote-notes/title").unwrap_or_default();
            let created_at = get_s("hulunote-notes/created-at").unwrap_or_default();
            let updated_at = get_s("hulunote-notes/updated-at").unwrap_or_default();

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
        // Canonical contract: create-note returns a note object.
        // Some deployments wrap it as {"note": {...}}, others return the note object directly.
        let v = data.get("note").cloned().unwrap_or(data);

        let get_s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());

        // Prefer non-namespaced keys if present.
        let id = get_s("id").unwrap_or_default();
        let database_id = get_s("database-id").unwrap_or_default();
        let title = get_s("title").unwrap_or_default();
        let content = get_s("content").unwrap_or_default();
        let created_at = get_s("created-at").unwrap_or_default();
        let updated_at = get_s("updated-at").unwrap_or_default();

        if !id.trim().is_empty() && !database_id.trim().is_empty() {
            return Some(Note {
                id,
                database_id,
                title,
                content,
                created_at,
                updated_at,
            });
        }

        // Namespaced note shape (as observed in hulunote deployments).
        let id = get_s("hulunote-notes/id").unwrap_or_default();
        let database_id = get_s("hulunote-notes/database-id").unwrap_or_default();
        let title = get_s("hulunote-notes/title").unwrap_or_default();
        let created_at = get_s("hulunote-notes/created-at").unwrap_or_default();
        let updated_at = get_s("hulunote-notes/updated-at").unwrap_or_default();

        if id.trim().is_empty() || database_id.trim().is_empty() {
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

        let payload = serde_json::json!({
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

    pub async fn update_note_title(&self, note_id: &str, title: &str) -> Result<(), String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/update-hulunote-note", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let payload = serde_json::json!({
            "note-id": note_id,
            "title": title,
        });

        let res = req.json(&payload).send().await.map_err(|e| e.to_string())?;

        if res.status().is_success() {
            Ok(())
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Update note failed ({status}): {body}"))
        }
    }

    fn parse_nav_list_response(data: serde_json::Value) -> Vec<Nav> {
        let list = data
            .get("nav-list")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut out: Vec<Nav> = Vec::with_capacity(list.len());
        for item in list {
            if let Ok(nav) = serde_json::from_value::<Nav>(item) {
                out.push(nav);
            }
        }
        out
    }

    pub async fn get_note_navs(&self, note_id: &str) -> Result<Vec<Nav>, String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/get-note-navs", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&GetNoteNavsRequest {
                note_id: note_id.to_string(),
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            Ok(Self::parse_nav_list_response(data))
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Failed to get navs ({status}): {body}"))
        }
    }

    pub async fn upsert_nav(
        &self,
        req_body: CreateOrUpdateNavRequest,
    ) -> Result<serde_json::Value, String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/create-or-update-nav", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req.json(&req_body).send().await.map_err(|e| e.to_string())?;

        if res.status().is_success() {
            res.json().await.map_err(|e| e.to_string())
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Upsert nav failed ({status}): {body}"))
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

        let res = client
            .post(format!("{}/login/web-signup", self.base_url))
            .json(&SignupRequest {
                email: email.to_string(),
                username: username.to_string(),
                password: password.to_string(),
                registration_code: registration_code.to_string(),
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

// Phase 5.5: local recents
const RECENT_DBS_KEY: &str = "hulunote_recent_dbs";
const RECENT_NOTES_KEY: &str = "hulunote_recent_notes";

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RecentDb {
    pub id: String,
    pub name: String,
    pub last_opened_ms: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RecentNote {
    pub db_id: String,
    pub note_id: String,
    pub title: String,
    pub last_opened_ms: i64,
}

fn now_ms() -> i64 {
    js_sys::Date::now().round() as i64
}

fn load_json_from_storage<T: for<'de> Deserialize<'de>>(key: &str) -> Option<T> {
    let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten())?;
    let json = storage.get_item(key).ok().flatten()?;
    serde_json::from_str(&json).ok()
}

fn save_json_to_storage<T: Serialize>(key: &str, value: &T) {
    if let Ok(json) = serde_json::to_string(value) {
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item(key, &json);
        }
    }
}

fn upsert_lru_by_key<T: Clone>(
    mut items: Vec<T>,
    item: T,
    same_key: impl Fn(&T, &T) -> bool,
    max: usize,
) -> Vec<T> {
    items.retain(|x| !same_key(x, &item));
    items.insert(0, item);
    if items.len() > max {
        items.truncate(max);
    }
    items
}

fn load_recent_dbs() -> Vec<RecentDb> {
    load_json_from_storage::<Vec<RecentDb>>(RECENT_DBS_KEY).unwrap_or_default()
}

fn load_recent_notes() -> Vec<RecentNote> {
    load_json_from_storage::<Vec<RecentNote>>(RECENT_NOTES_KEY).unwrap_or_default()
}

fn write_recent_db(id: &str, name: &str) {
    if id.trim().is_empty() {
        return;
    }

    let item = RecentDb {
        id: id.to_string(),
        name: name.to_string(),
        last_opened_ms: now_ms(),
    };

    let next = upsert_lru_by_key(load_recent_dbs(), item, |a, b| a.id == b.id, 10);
    save_json_to_storage(RECENT_DBS_KEY, &next);
}

fn write_recent_note(db_id: &str, note_id: &str, title: &str) {
    if db_id.trim().is_empty() || note_id.trim().is_empty() {
        return;
    }

    let item = RecentNote {
        db_id: db_id.to_string(),
        note_id: note_id.to_string(),
        title: title.to_string(),
        last_opened_ms: now_ms(),
    };

    let next = upsert_lru_by_key(
        load_recent_notes(),
        item,
        |a, b| a.db_id == b.db_id && a.note_id == b.note_id,
        20,
    );
    save_json_to_storage(RECENT_NOTES_KEY, &next);
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

#[derive(Clone)]
pub struct DbUiActions {
    pub open_create: Callback<()>,
    pub open_rename: Callback<(String, String)>,
    pub open_delete: Callback<(String, String)>,
}

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
pub fn HomeRecentsPage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let actions = expect_context::<DbUiActions>();
    let navigate = StoredValue::new(use_navigate());

    view! {
        <div class="space-y-3">
            <div class="space-y-1">
                <h1 class="text-xl font-semibold">"Databases"</h1>
            </div>

            <Show
                when=move || !app_state.0.databases.get().is_empty()
                fallback=|| view! { <div class="text-sm text-muted-foreground">"No databases."</div> }
            >
                <div class="grid gap-3 sm:grid-cols-2">
                    {move || {
                        use leptos::prelude::IntoAny;

                        let dbs = app_state.0.databases.get();

                        let placeholder = view! {
                            <Card
                                class="group relative flex h-40 cursor-pointer items-center justify-center border-dashed transition-colors hover:bg-surface-hover hover:ring-1 hover:ring-border"
                                on:click=move |_| actions.open_create.run(())
                            >
                                <div class="flex flex-col items-center gap-2 p-6">
                                    <div class="flex h-10 w-10 items-center justify-center rounded-full border border-border bg-background">
                                        <span class="text-lg text-muted-foreground">"+"</span>
                                    </div>
                                    <div class="text-sm font-medium">"New database"</div>
                                </div>
                            </Card>
                        }
                        .into_any();

                        dbs.into_iter()
                            .map(|db| {
                                let id = db.id.clone();
                                let name = db.name.clone();
                                let desc = db.description.clone();

                                let id_for_nav = id.clone();
                                let id_for_rename = id.clone();
                                let name_for_rename = name.clone();
                                let id_for_delete = id.clone();
                                let name_for_delete = name.clone();

                                view! {
                                    <Card
                                        class="group relative h-40 cursor-pointer transition-colors hover:bg-surface-hover hover:ring-1 hover:ring-border"
                                        on:click=move |_| {
                                            navigate.with_value(|nav| {
                                                nav(&format!("/db/{}", id_for_nav), Default::default());
                                            });
                                        }
                                    >
                                        <CardHeader class="p-4">
                                            <CardTitle class="truncate text-sm">{name}</CardTitle>
                                            <CardDescription class="line-clamp-2 text-xs">{desc}</CardDescription>
                                        </CardHeader>

                                        <div class="absolute bottom-2 right-2 hidden items-center gap-1 group-hover:flex">
                                            <Button
                                                variant=ButtonVariant::Ghost
                                                size=ButtonSize::Icon
                                                class="h-7 w-7"
                                                attr:title="Rename"
                                                on:click=move |ev: web_sys::MouseEvent| {
                                                    ev.stop_propagation();
                                                    actions.open_rename.run((id_for_rename.clone(), name_for_rename.clone()));
                                                }
                                            >
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
                                                    class="text-muted-foreground"
                                                    aria-hidden="true"
                                                >
                                                    <path d="M12 20h9" />
                                                    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z" />
                                                </svg>
                                            </Button>

                                            <Button
                                                variant=ButtonVariant::Ghost
                                                size=ButtonSize::Icon
                                                class="h-7 w-7 text-destructive"
                                                attr:title="Delete"
                                                on:click=move |ev: web_sys::MouseEvent| {
                                                    ev.stop_propagation();
                                                    actions
                                                        .open_delete
                                                        .run((id_for_delete.clone(), name_for_delete.clone()));
                                                }
                                            >
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
                                                    aria-hidden="true"
                                                >
                                                    <path d="M3 6h18" />
                                                    <path d="M8 6V4h8v2" />
                                                    <path d="M19 6l-1 14H6L5 6" />
                                                    <path d="M10 11v6" />
                                                    <path d="M14 11v6" />
                                                </svg>
                                            </Button>
                                        </div>
                                    </Card>
                                }
                                .into_any()
                            })
                            .chain(std::iter::once(placeholder))
                            .collect_view()
                    }}
                </div>
            </Show>
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

    // Home sidebar: rename/delete actions (hover)
    let rename_open: RwSignal<bool> = RwSignal::new(false);
    let rename_db_id: RwSignal<Option<String>> = RwSignal::new(None);
    let rename_value: RwSignal<String> = RwSignal::new(String::new());
    let rename_loading: RwSignal<bool> = RwSignal::new(false);
    let rename_error: RwSignal<Option<String>> = RwSignal::new(None);

    let delete_open: RwSignal<bool> = RwSignal::new(false);
    let delete_db_id: RwSignal<Option<String>> = RwSignal::new(None);
    let delete_db_name: RwSignal<String> = RwSignal::new(String::new());
    let delete_confirm: RwSignal<String> = RwSignal::new(String::new());
    let delete_loading: RwSignal<bool> = RwSignal::new(false);
    let delete_error: RwSignal<Option<String>> = RwSignal::new(None);

    let search_query = app_state.0.search_query;
    let search_ref: NodeRef<html::Input> = NodeRef::new();

    let navigate = StoredValue::new(use_navigate());
    let location = use_location();
    let pathname = move || location.pathname.get();
    let pathname_untracked = move || location.pathname.get_untracked();

    let sidebar_show_databases = move || {
        let p = pathname();
        // On Home, databases are shown in the main area (cards). In a DB, hide databases.
        !p.starts_with("/db/") && p != "/"
    };

    let sidebar_show_recent_notes = move || pathname() == "/";

    let sidebar_show_pages = move || {
        let p = pathname();
        p.starts_with("/db/")
    };

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

    let refresh_databases = move || {
        let mut c = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            if let Ok(dbs) = c.get_database_list().await {
                app_state.0.databases.set(dbs);
            }
            app_state.0.api_client.set(c);
        });
    };

    let on_open_rename_db = move |id: String, name: String| {
        rename_db_id.set(Some(id));
        rename_value.set(name);
        rename_error.set(None);
        rename_open.set(true);
    };

    let on_submit_rename_db = move |_: web_sys::MouseEvent| {
        if rename_loading.get_untracked() {
            return;
        }

        let id = rename_db_id.get_untracked().unwrap_or_default();
        let new_name = rename_value.get_untracked();
        if id.trim().is_empty() {
            return;
        }
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

    let on_open_delete_db = move |id: String, name: String| {
        delete_db_id.set(Some(id));
        delete_db_name.set(name);
        delete_confirm.set(String::new());
        delete_error.set(None);
        delete_open.set(true);
    };

    // Expose DB actions to pages (e.g. Home database cards).
    provide_context(DbUiActions {
        open_create: Callback::new(move |_| open_create_dialog()),
        open_rename: Callback::new(move |(id, name)| on_open_rename_db(id, name)),
        open_delete: Callback::new(move |(id, name)| on_open_delete_db(id, name)),
    });

    let on_submit_delete_db = move |_: web_sys::MouseEvent| {
        if delete_loading.get_untracked() {
            return;
        }

        let id = delete_db_id.get_untracked().unwrap_or_default();
        let name = delete_db_name.get_untracked();
        let confirm = delete_confirm.get_untracked();
        if id.trim().is_empty() {
            return;
        }
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
                    refresh_databases();
                    delete_open.set(false);

                    // If we are currently inside this DB, go Home.
                    if pathname_untracked().starts_with(&format!("/db/{id}")) {
                        navigate.with_value(|nav| nav("/", Default::default()));
                    }

                    // Clear selection if it matches.
                    if current_db_id.get_untracked().as_deref() == Some(id.as_str()) {
                        set_current_db(None);
                    }
                }
                Err(e) => delete_error.set(Some(e)),
            }
            delete_loading.set(false);
        });
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

    // If there is no selection yet, we only pick a default when the user is inside a DB route.
    // On Home, we intentionally do NOT highlight any database.
    Effect::new(move |_| {
        let selected = current_db_id.get();
        let dbs = databases.get();
        let p = pathname();

        if selected.is_none() && p.starts_with("/db/") {
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

                            <Show when=move || sidebar_show_recent_notes() fallback=|| ().into_view()>
                                <Card>
                                    <CardHeader class="p-3">
                                        <CardTitle class="text-sm">"Recent Notes"</CardTitle>
                                    </CardHeader>
                                    <CardContent class="p-3 pt-0">
                                        <Show
                                            when=move || !load_recent_notes().is_empty()
                                            fallback=|| view! { <div class="text-sm text-muted-foreground">"No recent notes."</div> }
                                        >
                                            <div class="space-y-1">
                                                {move || {
                                                    let dbs = expect_context::<AppContext>().0.databases.get();
                                                    load_recent_notes()
                                                        .into_iter()
                                                        .map(|n| {
                                                            let db_id = n.db_id.clone();
                                                            let db_id_href = db_id.clone();
                                                            let note_id = n.note_id.clone();
                                                            let title = n.title.clone();

                                                            let db_name_opt = dbs
                                                                .iter()
                                                                .find(|d| d.id == db_id)
                                                                .map(|d| d.name.clone());

                                                            view! {
                                                                <a
                                                                    href=format!("/db/{}/note/{}", db_id_href, note_id)
                                                                    class="block rounded-md border border-border bg-background px-3 py-2 transition-colors hover:bg-surface-hover"
                                                                >
                                                                    <div class="truncate text-sm font-medium">{title}</div>
                                                                    // Only show database name (never show raw id). Keep height stable.
                                                                    <div class="min-h-[1rem] truncate text-xs text-muted-foreground">
                                                                        {db_name_opt.unwrap_or_default()}
                                                                    </div>
                                                                </a>
                                                            }
                                                        })
                                                        .collect_view()
                                                }}
                                            </div>
                                        </Show>
                                    </CardContent>
                                </Card>
                            </Show>

                            <Show when=move || sidebar_show_databases() fallback=|| ().into_view()>
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
                                                    let allow_highlight = pathname().starts_with("/db/");
                                                    let show_actions = pathname() == "/";

                                                    databases
                                                        .get()
                                                        .into_iter()
                                                        .map(|db| {
                                                            let is_selected = allow_highlight
                                                                && selected.as_deref() == Some(db.id.as_str());
                                                            let variant = if is_selected {
                                                                ButtonVariant::Accent
                                                            } else {
                                                                ButtonVariant::Ghost
                                                            };

                                                            let id_href = db.id.clone();
                                                            let name_label = db.name.clone();
                                                            let name_for_rename = db.name.clone();
                                                            let name_for_delete = db.name.clone();
                                                            let id = db.id.clone();

                                                            view! {
                                                                <div class="group flex min-w-0 items-center gap-2">
                                                                    <Button
                                                                        variant=variant
                                                                        size=ButtonSize::Sm
                                                                        class="min-w-0 flex-1 justify-start"
                                                                        attr:aria-current=move || {
                                                                            if is_selected { Some("page") } else { None }
                                                                        }
                                                                        href=format!("/db/{}", id_href)
                                                                    >
                                                                        <span class="min-w-0 flex-1 truncate">{name_label}</span>
                                                                    </Button>

                                                                    <Show when=move || show_actions fallback=|| ().into_view()>
                                                                        <div class="hidden shrink-0 items-center gap-1 group-hover:flex">
                                                                            <Button
                                                                                variant=ButtonVariant::Ghost
                                                                                size=ButtonSize::Icon
                                                                                class="h-7 w-7"
                                                                                attr:title="Rename"
                                                                                on:click={
                                                                                    let id = id.clone();
                                                                                    let name = name_for_rename.clone();
                                                                                    move |ev: web_sys::MouseEvent| {
                                                                                        ev.stop_propagation();
                                                                                        on_open_rename_db(id.clone(), name.clone());
                                                                                    }
                                                                                }
                                                                            >
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
                                                                                    class="text-muted-foreground"
                                                                                    aria-hidden="true"
                                                                                >
                                                                                    <path d="M12 20h9" />
                                                                                    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z" />
                                                                                </svg>
                                                                            </Button>
                                                                            <Button
                                                                                variant=ButtonVariant::Ghost
                                                                                size=ButtonSize::Icon
                                                                                class="h-7 w-7 text-destructive"
                                                                                attr:title="Delete"
                                                                                on:click={
                                                                                    let id = id.clone();
                                                                                    let name = name_for_delete.clone();
                                                                                    move |ev: web_sys::MouseEvent| {
                                                                                        ev.stop_propagation();
                                                                                        on_open_delete_db(id.clone(), name.clone());
                                                                                    }
                                                                                }
                                                                            >
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
                                                                                    aria-hidden="true"
                                                                                >
                                                                                    <path d="M3 6h18" />
                                                                                    <path d="M8 6V4h8v2" />
                                                                                    <path d="M19 6l-1 14H6L5 6" />
                                                                                    <path d="M10 11v6" />
                                                                                    <path d="M14 11v6" />
                                                                                </svg>
                                                                            </Button>
                                                                        </div>
                                                                    </Show>
                                                                </div>
                                                            }
                                                        })
                                                        .collect_view()
                                                }}
                                            </Show>
                                        </div>
                                    </CardContent>
                                </Card>
                            </Show>

                            <Show when=move || sidebar_show_pages() fallback=|| ().into_view()>
                                <Card>
                                    <CardContent class="p-3">
                                        <span class="sr-only">"Pages"</span>
                                        <div class="space-y-1">
                                            {move || {
                                                let db_id = current_db_id.get().unwrap_or_default();
                                                let q = search_query.get().trim().to_lowercase();
                                                let notes = expect_context::<AppContext>().0.notes.get();

                                                // Highlight current note if we are on /db/:db_id/note/:note_id
                                                let p = pathname();
                                                let prefix = format!("/db/{}/note/", db_id);
                                                let current_note_id = p
                                                    .strip_prefix(&prefix)
                                                    .unwrap_or("")
                                                    .split('/')
                                                    .next()
                                                    .unwrap_or("");

                                                notes
                                                    .into_iter()
                                                    .filter(|n| n.database_id == db_id)
                                                    .filter(|n| {
                                                        if q.is_empty() {
                                                            true
                                                        } else {
                                                            n.title.to_lowercase().contains(&q)
                                                        }
                                                    })
                                                    .map(|n| {
                                                        let is_selected = n.id == current_note_id;
                                                        let variant = if is_selected {
                                                            ButtonVariant::Accent
                                                        } else {
                                                            ButtonVariant::Ghost
                                                        };
                                                        let id = n.id.clone();
                                                        view! {
                                                            <Button
                                                                variant=variant
                                                                size=ButtonSize::Sm
                                                                class="w-full justify-start"
                                                                attr:aria-current=move || if is_selected { Some("page") } else { None }
                                                                href=format!("/db/{}/note/{}", db_id, id)
                                                            >
                                                                {n.title}
                                                            </Button>
                                                        }
                                                    })
                                                    .collect_view()
                                            }}
                                        </div>
                                    </CardContent>
                                </Card>
                            </Show>

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
                    <div class="mb-4 flex items-center justify-between gap-3">
                        <nav class="min-w-0" aria-label="Breadcrumb">
                            {move || {
                                use leptos::prelude::IntoAny;

                                let p = pathname();

                                // Home
                                if p == "/" {
                                    return view! { <div class="truncate text-sm font-medium"></div> }
                                        .into_any();
                                }

                                // DB / Note
                                if p.starts_with("/db/") {
                                    let db_name = current_db_name()
                                        .unwrap_or_else(|| "Database".to_string());

                                    // If note route, show All databases > db > note
                                    if let Some(rest) = p.strip_prefix("/db/") {
                                        if let Some((db_id, tail)) = rest.split_once('/') {
                                            if let Some(_note_rest) = tail.strip_prefix("note/") {
                                                // Note route: do NOT show note title in breadcrumbs.
                                                return view! {
                                                    <div class="flex min-w-0 items-center gap-2 text-sm">
                                                        <a
                                                            href="/"
                                                            class="min-w-0 truncate font-medium text-foreground hover:underline"
                                                        >
                                                            "All databases"
                                                        </a>
                                                        <span class="text-muted-foreground">"›"</span>
                                                        <a
                                                            href=format!("/db/{}", db_id)
                                                            class="min-w-0 truncate font-medium text-foreground hover:underline"
                                                        >
                                                            {db_name}
                                                        </a>
                                                    </div>
                                                }
                                                .into_any();
                                            }

                                            // DB home: All databases > db
                                            return view! {
                                                <div class="flex min-w-0 items-center gap-2 text-sm">
                                                    <a
                                                        href="/"
                                                        class="min-w-0 truncate font-medium text-foreground hover:underline"
                                                    >
                                                        "All databases"
                                                    </a>
                                                    <span class="text-muted-foreground">"›"</span>
                                                    <div class="min-w-0 truncate font-medium">{db_name}</div>
                                                </div>
                                            }
                                            .into_any();
                                        }
                                    }

                                    // Fallback DB home
                                    return view! {
                                        <div class="flex min-w-0 items-center gap-2 text-sm">
                                            <a
                                                href="/"
                                                class="min-w-0 truncate font-medium text-foreground hover:underline"
                                            >
                                                "All databases"
                                            </a>
                                            <span class="text-muted-foreground">"›"</span>
                                            <div class="min-w-0 truncate font-medium">{db_name}</div>
                                        </div>
                                    }
                                    .into_any();
                                }

                                // Fallback
                                view! { <div class="truncate text-sm font-medium">"Hulunote"</div> }.into_any()
                            }}
                        </nav>

                        <div class="flex shrink-0 items-center gap-2"></div>
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
                                        on:click=on_submit_rename_db
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
                                <div class="text-sm font-medium text-destructive">"Delete database"</div>
                                <div class="text-xs text-muted-foreground">
                                    "Type the database name to confirm deletion."
                                </div>
                            </div>

                            <div class="space-y-2">
                                <div class="rounded-md border border-border bg-muted px-3 py-2 text-sm">
                                    {move || delete_db_name.get()}
                                </div>

                                <div class="space-y-1">
                                    <Label class="text-xs">"Confirm name"</Label>
                                    <Input bind_value=delete_confirm class="h-8 text-sm" placeholder="Type name exactly" />
                                </div>

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
                                        variant=ButtonVariant::Outline
                                        size=ButtonSize::Sm
                                        class="border-destructive/40 text-destructive"
                                        attr:disabled=move || delete_loading.get()
                                        on:click=on_submit_delete_db
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
    view! {
        <RootAuthed>
            <HomeRecentsPage />
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
    let app_state = expect_context::<AppContext>();
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

    let title_value: RwSignal<String> = RwSignal::new(String::new());
    // Track which note the title_value currently belongs to.
    let title_note_id: RwSignal<String> = RwSignal::new(String::new());

    // Keep global selected DB in sync when entering a note route directly (e.g. from Home recents).
    Effect::new(move |_| {
        let db = db_id();
        if db.trim().is_empty() {
            return;
        }

        if app_state.0.current_database_id.get() != Some(db.clone()) {
            app_state.0.current_database_id.set(Some(db.clone()));

            // Persist selection for future sessions.
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten())
            {
                let _ = storage.set_item(CURRENT_DB_KEY, &db);
            }
        }
    });

    let saving: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);

    // Ensure notes for this DB are loaded when deep-linking directly into a note page.
    // This prevents recent-note title from falling back to note_id.
    Effect::new(move |_| {
        let db = db_id();
        let id = note_id();
        if db.trim().is_empty() || id.trim().is_empty() {
            return;
        }

        let already_loaded_db =
            app_state.0.notes_last_loaded_db_id.get().as_deref() == Some(db.as_str());
        let has_note = app_state.0.notes.get().into_iter().any(|n| n.id == id);
        let is_loading = app_state.0.notes_loading.get();

        if (!already_loaded_db || !has_note) && !is_loading {
            // Kick off a load with stale-response protection.
            app_state.0.notes_last_loaded_db_id.set(Some(db.clone()));

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
                let result = api_client.get_all_note_list(&db).await;

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
                        }
                    }
                }
                app_state.0.notes_loading.set(false);
            });
        }
    });

    // Keep local edit state in sync with loaded notes + write recent note.
    Effect::new(move |_| {
        let id = note_id();
        let db = db_id();
        if id.trim().is_empty() || db.trim().is_empty() {
            return;
        }

        if let Some(n) = app_state.0.notes.get().into_iter().find(|n| n.id == id) {
            // If we navigated to a different note, sync title input immediately.
            if title_note_id.get() != id {
                title_note_id.set(id.clone());
                title_value.set(n.title.clone());
            } else if title_value.get().trim().is_empty() {
                // Only overwrite local input when it's empty (avoid clobbering user typing).
                title_value.set(n.title.clone());
            }

            // Phase 5.5: recent notes (local)
            write_recent_note(&db, &id, &n.title);
        } else {
            // Fallback: at least record ids.
            write_recent_note(&db, &id, &id);
        }

        // Keep recent DB fresh too.
        if let Some(d) = app_state.0.databases.get().into_iter().find(|d| d.id == db) {
            write_recent_db(&d.id, &d.name);
        } else {
            write_recent_db(&db, &db);
        }
    });

    let save_title = move || {
        if saving.get_untracked() {
            return;
        }
        let id = note_id();
        let db = db_id();
        let new_title = title_value.get_untracked();
        if id.trim().is_empty() {
            return;
        }
        if new_title.trim().is_empty() {
            error.set(Some("Title cannot be empty".to_string()));
            return;
        }

        saving.set(true);
        error.set(None);

        let api_client = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            match api_client.update_note_title(&id, &new_title).await {
                Ok(_) => {
                    // Refresh notes list.
                    let c = app_state.0.api_client.get_untracked();
                    if let Ok(notes) = c.get_all_note_list(&db).await {
                        app_state.0.notes.set(notes);
                    }
                    app_state.0.api_client.set(c);
                }
                Err(e) => {
                    error.set(Some(e));
                }
            }
            saving.set(false);
        });
    };

    let _current_note = move || {
        let id = note_id();
        app_state.0.notes.get().into_iter().find(|n| n.id == id)
    };

    view! {
        <div class="space-y-3">
            <div class="space-y-2">
                <div class="flex items-center gap-2">
                    <Input
                        bind_value=title_value
                        class="h-10 min-w-0 flex-1 text-lg font-semibold"
                        placeholder="Untitled"
                        on:blur=move |_| save_title()
                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                save_title();

                                // UX: pressing Enter should commit and exit the title field.
                                if let Some(t) = ev
                                    .target()
                                    .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                {
                                    let _ = t.blur();
                                }
                            }
                        }
                    />

                    // Reserve space to avoid layout shift/flicker.
                    <div class="h-5 w-5 shrink-0">
                        <Show when=move || saving.get() fallback=|| ().into_view()>
                            <div class="h-5 w-5">
                                <Spinner />
                            </div>
                        </Show>
                    </div>
                </div>

                <Show when=move || error.get().is_some() fallback=|| ().into_view()>
                    {move || error.get().map(|e| view! {
                        <Alert class="border-destructive/30">
                            <AlertDescription class="text-destructive text-xs">{e}</AlertDescription>
                        </Alert>
                    })}
                </Show>

                <OutlineEditor note_id=note_id />
            </div>
        </div>
    }
}

#[component]
pub fn OutlineEditor(note_id: impl Fn() -> String + Clone + Send + Sync + 'static) -> impl IntoView {
    let app_state = expect_context::<AppContext>();

    let navs: RwSignal<Vec<Nav>> = RwSignal::new(vec![]);
    let loading: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);

    // Editing state
    let editing_id: RwSignal<Option<String>> = RwSignal::new(None);
    let editing_value: RwSignal<String> = RwSignal::new(String::new());
    let target_cursor_col: RwSignal<Option<u32>> = RwSignal::new(None);
    let editing_ref: NodeRef<html::Input> = NodeRef::new();

    // Load navs when note_id changes.
    let note_id_for_effect = note_id.clone();
    Effect::new(move |_| {
        let id = note_id_for_effect();
        if id.trim().is_empty() {
            navs.set(vec![]);
            return;
        }

        loading.set(true);
        error.set(None);

        let api_client = app_state.0.api_client.get_untracked();
        spawn_local(async move {
            match api_client.get_note_navs(&id).await {
                Ok(list) => navs.set(list),
                Err(e) => error.set(Some(e)),
            }
            loading.set(false);
        });
    });

    // Focus the inline editor when editing_id changes.
    Effect::new(move |_| {
        let id = editing_id.get();
        if id.is_none() {
            return;
        }

        let col = target_cursor_col.get_untracked();
        let el = editing_ref.get();
        if let Some(el) = el {
            // Focus on next tick so the node is mounted.
            let _ = web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    wasm_bindgen::closure::Closure::once_into_js(move || {
                        let _ = el.focus();
                        if let Some(col) = col {
                            // selectionStart/End are in UTF-16 code units.
                            let len = el.value().encode_utf16().count() as u32;
                            let pos = col.min(len);
                            let _ = el.set_selection_range(pos, pos);
                        }
                    })
                    .as_ref()
                    .unchecked_ref(),
                    0,
                );
        }
    });

    view! {
        <div class="rounded-md border bg-card p-3">
            <div class="text-xs text-muted-foreground">"Outline"</div>

            <Show when=move || loading.get() fallback=|| ().into_view()>
                <div class="mt-2"><Spinner /></div>
            </Show>

            <Show when=move || error.get().is_some() fallback=|| ().into_view()>
                {move || error.get().map(|e| view! {
                    <div class="mt-2 text-xs text-destructive">{e}</div>
                })}
            </Show>

            <div class="mt-2">
                {move || {
                    let all = navs.get();
                    let root = "00000000-0000-0000-0000-000000000000";

                    let mut roots = all
                        .iter()
                        .filter(|n| n.parid == root)
                        .cloned()
                        .collect::<Vec<_>>();
                    roots.sort_by(|a, b| a
                        .same_deep_order
                        .partial_cmp(&b.same_deep_order)
                        .unwrap_or(std::cmp::Ordering::Equal));

                    if roots.is_empty() {
                        view! { <div class="text-xs text-muted-foreground">"No nodes"</div> }
                            .into_any()
                    } else {
                        let nid = note_id();
                        view! {
                            <div class="space-y-0.5">
                                {roots
                                    .into_iter()
                                    .map(|n| view! {
                                        <OutlineNode
                                            nav_id=n.id
                                            depth=0
                                            navs=navs
                                            note_id=nid.clone()
                                            editing_id=editing_id
                                            editing_value=editing_value
                                            target_cursor_col=target_cursor_col
                                            editing_ref=editing_ref
                                        />
                                    })
                                    .collect_view()}
                            </div>
                        }
                        .into_any()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
pub fn OutlineNode(
    nav_id: String,
    depth: usize,
    navs: RwSignal<Vec<Nav>>,
    note_id: String,
    editing_id: RwSignal<Option<String>>,
    editing_value: RwSignal<String>,
    target_cursor_col: RwSignal<Option<u32>>,
    editing_ref: NodeRef<html::Input>,
) -> impl IntoView {
    let app_state = expect_context::<AppContext>();

    let nav_id_for_nav = nav_id.clone();
    let nav_id_for_toggle = nav_id.clone();
    let nav_id_for_render = nav_id.clone();
    let note_id_for_toggle = note_id.clone();

    let nav_id_sv = StoredValue::new(nav_id.clone());
    let note_id_sv = StoredValue::new(note_id.clone());

    let nav = move || navs.get().into_iter().find(|n| n.id == nav_id_for_nav);

    let on_toggle = Callback::new(move |_| {
        let Some(n) = navs
            .get_untracked()
            .into_iter()
            .find(|n| n.id == nav_id_for_toggle) else {
            return;
        };

        let new_display = !n.is_display;
        navs.update(|xs| {
            if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_for_toggle) {
                x.is_display = new_display;
            }
        });

        let api_client = app_state.0.api_client.get_untracked();
        let req = CreateOrUpdateNavRequest {
            note_id: note_id_for_toggle.clone(),
            id: Some(nav_id_for_toggle.clone()),
            parid: None,
            content: None,
            order: None,
            is_display: Some(new_display),
            is_delete: None,
            properties: None,
        };
        spawn_local(async move {
            let _ = api_client.upsert_nav(req).await;
        });
    });

    let indent_px = (depth * 18) as i32;

    view! {
        <div>
            {move || {
                let Some(n) = nav() else {
                    return ().into_view().into_any();
                };

                // Compute children for this render.
                let mut kids = navs
                    .get()
                    .into_iter()
                    .filter(|x| x.parid == nav_id_for_render)
                    .collect::<Vec<_>>();
                kids.sort_by(|a, b| {
                    a.same_deep_order
                        .partial_cmp(&b.same_deep_order)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let has_kids = !kids.is_empty();
                let bullet = if has_kids {
                    if n.is_display { "▾" } else { "▸" }
                } else {
                    "·"
                };

                let on_toggle_cb = on_toggle.clone();

                let children_view = if n.is_display && has_kids {
                    kids.into_iter()
                        .map(|c| view! {
                            <OutlineNode
                                nav_id=c.id
                                depth=depth+1
                                navs=navs
                                note_id=note_id.clone()
                                editing_id=editing_id
                                editing_value=editing_value
                                target_cursor_col=target_cursor_col
                                editing_ref=editing_ref
                            />
                        })
                        .collect_view()
                        .into_any()
                } else {
                    ().into_view().into_any()
                };

                view! {
                    <div>
                        <div
                            class="flex items-start gap-2 py-1"
                            style=move || format!("padding-left: {}px", indent_px)
                        >
                            <button
                                class="mt-1 h-4 w-4 text-xs text-muted-foreground"
                                on:click=move |ev| on_toggle_cb.run(ev)
                                disabled=!has_kids
                            >
                                {bullet}
                            </button>

                            <div class="min-w-0 flex-1 text-sm">
                                {move || {
                                    let id = nav_id_sv.get_value();
                                    let is_editing = editing_id.get().as_deref() == Some(id.as_str());

                                    if !is_editing {
                                        let content_now = n.content.clone();
                                        let content_for_click = content_now.clone();

                                        // Ensure empty nodes still have a clickable area.
                                        let content_display = if content_now.trim().is_empty() {
                                            "\u{00A0}".to_string()
                                        } else {
                                            content_now
                                        };

                                        return view! {
                                            <div
                                                class="cursor-text whitespace-pre-wrap min-h-[20px]"
                                                on:click=move |_| {
                                                    let id = nav_id_sv.get_value();
                                                    editing_id.set(Some(id));
                                                    editing_value.set(content_for_click.clone());
                                                }
                                            >
                                                {content_display}
                                            </div>
                                        }
                                        .into_any();
                                    }

                                    view! {
                                        <input
                                            node_ref=editing_ref
                                            class="h-7 w-full min-w-0 flex-1 rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50"
                                            value=move || editing_value.get()
                                            on:input=move |ev| {
                                                editing_value.set(event_target_value(&ev));
                                            }
                                            on:blur=move |_| {
                                                let new_content = editing_value.get_untracked();
                                                let nav_id_now = nav_id_sv.get_value();
                                                let note_id_now = note_id_sv.get_value();

                                                // IMPORTANT: read StoredValue first; setting editing_id may unmount this node.
                                                editing_id.set(None);

                                                navs.update(|xs| {
                                                    if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                        x.content = new_content.clone();
                                                    }
                                                });

                                                let api_client = app_state.0.api_client.get_untracked();
                                                let req = CreateOrUpdateNavRequest {
                                                    note_id: note_id_now,
                                                    id: Some(nav_id_now.clone()),
                                                    parid: None,
                                                    content: Some(new_content),
                                                    order: None,
                                                    is_display: None,
                                                    is_delete: None,
                                                    properties: None,
                                                };
                                                spawn_local(async move {
                                                    let _ = api_client.upsert_nav(req).await;
                                                });
                                            }
                                            on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                let key = ev.key();

                                                // Helpers for Roam-style navigation
                                                let input = || {
                                                    ev.target()
                                                        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                                };

                                                let save_current = |nav_id_now: &str, note_id_now: &str| {
                                                    let current_content = editing_value.get_untracked();
                                                    navs.update(|xs| {
                                                        if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                            x.content = current_content.clone();
                                                        }
                                                    });

                                                    let api_client = app_state.0.api_client.get_untracked();
                                                    let save_req = CreateOrUpdateNavRequest {
                                                        note_id: note_id_now.to_string(),
                                                        id: Some(nav_id_now.to_string()),
                                                        parid: None,
                                                        content: Some(current_content),
                                                        order: None,
                                                        is_display: None,
                                                        is_delete: None,
                                                        properties: None,
                                                    };

                                                    spawn_local(async move {
                                                        let _ = api_client.upsert_nav(save_req).await;
                                                    });
                                                };

                                                fn visible_preorder(all: &[Nav]) -> Vec<String> {
                                                    let root = "00000000-0000-0000-0000-000000000000";

                                                    fn children_sorted(all: &[Nav], parid: &str) -> Vec<Nav> {
                                                        let mut out = all
                                                            .iter()
                                                            .filter(|n| n.parid == parid)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        out.sort_by(|a, b| {
                                                            a.same_deep_order
                                                                .partial_cmp(&b.same_deep_order)
                                                                .unwrap_or(std::cmp::Ordering::Equal)
                                                        });
                                                        out
                                                    }

                                                    fn collect(all: &[Nav], parid: &str, out: &mut Vec<String>) {
                                                        for n in children_sorted(all, parid) {
                                                            out.push(n.id.clone());
                                                            if n.is_display {
                                                                collect(all, &n.id, out);
                                                            }
                                                        }
                                                    }

                                                    let mut out: Vec<String> = vec![];
                                                    collect(all, root, &mut out);
                                                    out
                                                }

                                                // Arrow Up/Down: move between visible nodes
                                                if key == "ArrowUp" || key == "ArrowDown" {
                                                    ev.prevent_default();

                                                    let cursor_col = input()
                                                        .as_ref()
                                                        .and_then(|i| i.selection_start().ok().flatten())
                                                        .unwrap_or(0);
                                                    target_cursor_col.set(Some(cursor_col));

                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();
                                                    save_current(&nav_id_now, &note_id_now);

                                                    let all = navs.get_untracked();
                                                    let visible = visible_preorder(&all);

                                                    let idx = visible.iter().position(|id| id == &nav_id_now);
                                                    let Some(idx) = idx else { return; };

                                                    let next_id = if key == "ArrowUp" {
                                                        if idx == 0 { None } else { Some(visible[idx - 1].clone()) }
                                                    } else {
                                                        if idx + 1 >= visible.len() { None } else { Some(visible[idx + 1].clone()) }
                                                    };

                                                    if let Some(next_id) = next_id {
                                                        if let Some(next_nav) = all.iter().find(|n| n.id == next_id) {
                                                            editing_id.set(Some(next_id));
                                                            editing_value.set(next_nav.content.clone());
                                                        }
                                                    }

                                                    return;
                                                }

                                                // Arrow Left/Right: jump to prev/next visible node at boundaries
                                                if key == "ArrowLeft" || key == "ArrowRight" {
                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    let (cursor_start, cursor_end, len) = if let Some(i) = input() {
                                                        let start = i.selection_start().ok().flatten().unwrap_or(0);
                                                        let end = i.selection_end().ok().flatten().unwrap_or(start);
                                                        // IMPORTANT: selectionStart/End use UTF-16 code units, not Rust UTF-8 bytes.
                                                        let len = i.value().encode_utf16().count() as u32;
                                                        (start, end, len)
                                                    } else {
                                                        (0, 0, 0)
                                                    };

                                                    // Only trigger when selection is collapsed.
                                                    if cursor_start != cursor_end {
                                                        return;
                                                    }

                                                    if key == "ArrowLeft" && cursor_start == 0 {
                                                        ev.prevent_default();
                                                        target_cursor_col.set(None);
                                                        save_current(&nav_id_now, &note_id_now);

                                                        let all = navs.get_untracked();
                                                        let Some(me) = all.iter().find(|n| n.id == nav_id_now) else {
                                                            return;
                                                        };

                                                        let root = "00000000-0000-0000-0000-000000000000";

                                                        // Prefer previous sibling when it exists.
                                                        // If there is no previous sibling (i.e. first child), go to parent.
                                                        let parid = me.parid.clone();
                                                        let mut sibs = all
                                                            .iter()
                                                            .filter(|n| n.parid == parid)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        sibs.sort_by(|a, b| a
                                                            .same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal));

                                                        let prev = sibs
                                                            .iter()
                                                            .rev()
                                                            .find(|s| s.same_deep_order < me.same_deep_order)
                                                            .cloned();

                                                        if prev.is_none() {
                                                            if me.parid != root {
                                                                if let Some(parent) = all.iter().find(|n| n.id == me.parid) {
                                                                    editing_id.set(Some(parent.id.clone()));
                                                                    editing_value.set(parent.content.clone());
                                                                    target_cursor_col.set(Some(parent.content.encode_utf16().count() as u32));
                                                                }
                                                            }
                                                            return;
                                                        }

                                                        let prev = prev.unwrap();

                                                        // Descend to last visible node in prev's subtree.
                                                        fn last_visible_descendant(all: &[Nav], start: &Nav) -> Nav {
                                                            if !start.is_display {
                                                                return start.clone();
                                                            }
                                                            let mut children = all
                                                                .iter()
                                                                .filter(|n| n.parid == start.id)
                                                                .cloned()
                                                                .collect::<Vec<_>>();
                                                            children.sort_by(|a, b| a
                                                                .same_deep_order
                                                                .partial_cmp(&b.same_deep_order)
                                                                .unwrap_or(std::cmp::Ordering::Equal));
                                                            if let Some(last) = children.last() {
                                                                return last_visible_descendant(all, last);
                                                            }
                                                            start.clone()
                                                        }

                                                        let target = last_visible_descendant(&all, &prev);
                                                        editing_id.set(Some(target.id.clone()));
                                                        editing_value.set(target.content.clone());
                                                        target_cursor_col.set(Some(target.content.encode_utf16().count() as u32));
                                                        return;
                                                    }

                                                    if key == "ArrowRight" && cursor_start == len {
                                                        ev.prevent_default();
                                                        target_cursor_col.set(None);
                                                        save_current(&nav_id_now, &note_id_now);

                                                        let all = navs.get_untracked();

                                                        // Roam-ish behavior: if node has children and is collapsed, expand.
                                                        // If expanded, move into first child.
                                                        let mut children = all
                                                            .iter()
                                                            .filter(|n| n.parid == nav_id_now)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        children.sort_by(|a, b| a
                                                            .same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal));

                                                        if let Some(first_child) = children.first().cloned() {
                                                            let is_display = all
                                                                .iter()
                                                                .find(|n| n.id == nav_id_now)
                                                                .map(|n| n.is_display)
                                                                .unwrap_or(true);

                                                            if !is_display {
                                                                // Expand current node AND descend into first child.
                                                                // This matches Roam's feel: Right at end opens and goes deeper.
                                                                navs.update(|xs| {
                                                                    if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                                        x.is_display = true;
                                                                    }
                                                                });

                                                                let api_client = app_state.0.api_client.get_untracked();
                                                                let req = CreateOrUpdateNavRequest {
                                                                    note_id: note_id_now.clone(),
                                                                    id: Some(nav_id_now.clone()),
                                                                    parid: None,
                                                                    content: None,
                                                                    order: None,
                                                                    is_display: Some(true),
                                                                    is_delete: None,
                                                                    properties: None,
                                                                };
                                                                spawn_local(async move {
                                                                    let _ = api_client.upsert_nav(req).await;
                                                                });

                                                                editing_id.set(Some(first_child.id.clone()));
                                                                editing_value.set(first_child.content.clone());
                                                                target_cursor_col.set(Some(0));
                                                                return;
                                                            }

                                                            // Move into first child.
                                                            editing_id.set(Some(first_child.id.clone()));
                                                            editing_value.set(first_child.content.clone());
                                                            target_cursor_col.set(Some(0));
                                                            return;
                                                        }

                                                        // Strict Roam behavior: if there are no children, ArrowRight does not move to a sibling.
                                                        return;
                                                    }
                                                }

                                                // Tab / Shift+Tab: indent / outdent
                                                if key == "Tab" {
                                                    ev.prevent_default();

                                                    let shift = ev.shift_key();
                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    let all = navs.get_untracked();
                                                    let Some(me) = all.iter().find(|x| x.id == nav_id_now) else {
                                                        return;
                                                    };

                                                    // Save current edit buffer into local state first.
                                                    let current_content = editing_value.get_untracked();
                                                    navs.update(|xs| {
                                                        if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                            x.content = current_content.clone();
                                                        }
                                                    });

                                                    let api_client = app_state.0.api_client.get_untracked();

                                                    if !shift {
                                                        // Indent: become child of previous sibling.
                                                        let parid = me.parid.clone();
                                                        let mut sibs = all
                                                            .iter()
                                                            .filter(|x| x.parid == parid)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        sibs.sort_by(|a, b| a.same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal));

                                                        let prev = sibs
                                                            .iter()
                                                            .rev()
                                                            .find(|s| s.same_deep_order < me.same_deep_order)
                                                            .cloned();

                                                        let Some(prev) = prev else {
                                                            return;
                                                        };

                                                        let new_parid = prev.id.clone();

                                                        // Append to end of new parent's children.
                                                        let last_child_order = all
                                                            .iter()
                                                            .filter(|x| x.parid == new_parid)
                                                            .map(|x| x.same_deep_order)
                                                            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                                                        let new_order = last_child_order.unwrap_or(0.0) + 1.0;

                                                        navs.update(|xs| {
                                                            if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                                x.parid = new_parid.clone();
                                                                x.same_deep_order = new_order;
                                                            }
                                                            if let Some(p) = xs.iter_mut().find(|x| x.id == new_parid) {
                                                                p.is_display = true;
                                                            }
                                                        });

                                                        let req = CreateOrUpdateNavRequest {
                                                            note_id: note_id_now,
                                                            id: Some(nav_id_now.clone()),
                                                            parid: Some(new_parid),
                                                            content: Some(current_content),
                                                            order: Some(new_order),
                                                            is_display: None,
                                                            is_delete: None,
                                                            properties: None,
                                                        };

                                                        spawn_local(async move {
                                                            let _ = api_client.upsert_nav(req).await;
                                                        });
                                                    } else {
                                                        // Outdent: become sibling of parent.
                                                        let parent_id = me.parid.clone();
                                                        let root = "00000000-0000-0000-0000-000000000000";
                                                        if parent_id == root {
                                                            return;
                                                        }

                                                        let Some(parent) = all.iter().find(|x| x.id == parent_id) else {
                                                            return;
                                                        };

                                                        let new_parid = parent.parid.clone();

                                                        // Put right after parent (midpoint between parent and parent's next sibling).
                                                        let mut parent_sibs = all
                                                            .iter()
                                                            .filter(|x| x.parid == new_parid)
                                                            .cloned()
                                                            .collect::<Vec<_>>();
                                                        parent_sibs.sort_by(|a, b| a.same_deep_order
                                                            .partial_cmp(&b.same_deep_order)
                                                            .unwrap_or(std::cmp::Ordering::Equal));

                                                        let next_order = parent_sibs
                                                            .iter()
                                                            .find(|s| s.same_deep_order > parent.same_deep_order)
                                                            .map(|s| s.same_deep_order);

                                                        let new_order = if let Some(no) = next_order {
                                                            (parent.same_deep_order + no) / 2.0
                                                        } else {
                                                            parent.same_deep_order + 1.0
                                                        };

                                                        navs.update(|xs| {
                                                            if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                                x.parid = new_parid.clone();
                                                                x.same_deep_order = new_order;
                                                            }
                                                        });

                                                        let req = CreateOrUpdateNavRequest {
                                                            note_id: note_id_now,
                                                            id: Some(nav_id_now.clone()),
                                                            parid: Some(new_parid),
                                                            content: Some(current_content),
                                                            order: Some(new_order),
                                                            is_display: None,
                                                            is_delete: None,
                                                            properties: None,
                                                        };

                                                        spawn_local(async move {
                                                            let _ = api_client.upsert_nav(req).await;
                                                        });
                                                    }

                                                    // Keep editing current node.
                                                    editing_id.set(Some(nav_id_now));
                                                    return;
                                                }

                                                // Backspace/Delete on empty: soft-delete node (and its subtree)
                                                if (key == "Backspace" || key == "Delete")
                                                    && editing_value.get_untracked().trim().is_empty()
                                                {
                                                    ev.prevent_default();

                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    let all = navs.get_untracked();

                                                    // Visible order for choosing next focus.
                                                    let visible = visible_preorder(&all);
                                                    let idx = visible.iter().position(|id| id == &nav_id_now);

                                                    // Collect subtree ids (including self).
                                                    fn collect_subtree(all: &[Nav], root_id: &str, out: &mut Vec<String>) {
                                                        out.push(root_id.to_string());
                                                        for c in all.iter().filter(|n| n.parid == root_id) {
                                                            collect_subtree(all, &c.id, out);
                                                        }
                                                    }

                                                    let mut subtree: Vec<String> = vec![];
                                                    collect_subtree(&all, &nav_id_now, &mut subtree);

                                                    // Update local state: remove subtree nodes.
                                                    navs.update(|xs| xs.retain(|n| !subtree.iter().any(|id| id == &n.id)));

                                                    // Pick next focus: previous visible if possible, else next.
                                                    let next_focus = idx
                                                        .and_then(|i| if i > 0 { Some(visible[i - 1].clone()) } else { None })
                                                        .or_else(|| idx.and_then(|i| visible.get(i + 1).cloned()));

                                                    editing_id.set(next_focus.clone());
                                                    if let Some(fid) = next_focus {
                                                        if let Some(n) = all.iter().find(|n| n.id == fid) {
                                                            editing_value.set(n.content.clone());
                                                            target_cursor_col.set(Some(n.content.encode_utf16().count() as u32));
                                                        }
                                                    } else {
                                                        editing_id.set(None);
                                                    }

                                                    // Persist soft delete to backend.
                                                    let api_client = app_state.0.api_client.get_untracked();
                                                    spawn_local(async move {
                                                        for id in subtree {
                                                            let req = CreateOrUpdateNavRequest {
                                                                note_id: note_id_now.clone(),
                                                                id: Some(id),
                                                                parid: None,
                                                                content: None,
                                                                order: None,
                                                                is_display: None,
                                                                is_delete: Some(true),
                                                                properties: None,
                                                            };
                                                            let _ = api_client.upsert_nav(req).await;
                                                        }
                                                    });

                                                    return;
                                                }

                                                // Enter: save + create next sibling
                                                if key == "Enter" {
                                                    ev.prevent_default();

                                                    let current_content = editing_value.get_untracked();
                                                    let nav_id_now = nav_id_sv.get_value();
                                                    let note_id_now = note_id_sv.get_value();

                                                    navs.update(|xs| {
                                                        if let Some(x) = xs.iter_mut().find(|x| x.id == nav_id_now) {
                                                            x.content = current_content.clone();
                                                        }
                                                    });

                                                    let api_client = app_state.0.api_client.get_untracked();
                                                    let save_req = CreateOrUpdateNavRequest {
                                                        note_id: note_id_now.clone(),
                                                        id: Some(nav_id_now.clone()),
                                                        parid: None,
                                                        content: Some(current_content),
                                                        order: None,
                                                        is_display: None,
                                                        is_delete: None,
                                                        properties: None,
                                                    };

                                                    // Create sibling
                                                    let all = navs.get_untracked();
                                                    let Some(me) = all.iter().find(|x| x.id == nav_id_now) else {
                                                        return;
                                                    };

                                                    let parid = me.parid.clone();
                                                    let mut sibs = all
                                                        .iter()
                                                        .filter(|x| x.parid == parid)
                                                        .cloned()
                                                        .collect::<Vec<_>>();
                                                    sibs.sort_by(|a, b| a.same_deep_order
                                                        .partial_cmp(&b.same_deep_order)
                                                        .unwrap_or(std::cmp::Ordering::Equal));

                                                    let next_order = sibs
                                                        .iter()
                                                        .find(|s| s.same_deep_order > me.same_deep_order)
                                                        .map(|s| s.same_deep_order);

                                                    let new_order = if let Some(no) = next_order {
                                                        (me.same_deep_order + no) / 2.0
                                                    } else {
                                                        me.same_deep_order + 1.0
                                                    };

                                                    editing_id.set(None);

                                                    spawn_local(async move {
                                                        let _ = api_client.upsert_nav(save_req).await;

                                                        let create_req = CreateOrUpdateNavRequest {
                                                            note_id: note_id_now.clone(),
                                                            id: None,
                                                            parid: Some(parid.clone()),
                                                            content: Some("".to_string()),
                                                            order: Some(new_order),
                                                            is_display: Some(true),
                                                            is_delete: Some(false),
                                                            properties: None,
                                                        };

                                                        if let Ok(resp) = api_client.upsert_nav(create_req).await {
                                                            let new_id = resp
                                                                .get("id")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("")
                                                                .to_string();

                                                            if !new_id.trim().is_empty() {
                                                                navs.update(|xs| {
                                                                    xs.push(Nav {
                                                                        id: new_id.clone(),
                                                                        note_id: note_id_now.clone(),
                                                                        parid: parid.clone(),
                                                                        same_deep_order: new_order,
                                                                        content: String::new(),
                                                                        is_display: true,
                                                                        is_delete: false,
                                                                    });
                                                                    xs.sort_by(|a, b| a.same_deep_order
                                                                        .partial_cmp(&b.same_deep_order)
                                                                        .unwrap_or(std::cmp::Ordering::Equal));
                                                                });
                                                                editing_id.set(Some(new_id));
                                                                editing_value.set(String::new());
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                        />
                                    }
                                    .into_any()
                                }}
                            </div>
                        </div>

                        {children_view}
                    </div>
                }
                .into_any()
            }}
        </div>
    }
}

#[component]
pub fn DbHomePage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let params = leptos_router::hooks::use_params::<DbRouteParams>();
    let navigate = StoredValue::new(use_navigate());
    let location = use_location();
    let pathname = move || location.pathname.get();

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

    // Keep global selection in sync with URL + write recent DB.
    Effect::new(move |_| {
        let id = db_id();
        if id.trim().is_empty() {
            return;
        }

        if app_state.0.current_database_id.get() != Some(id.clone()) {
            app_state.0.current_database_id.set(Some(id.clone()));
            persist_current_db(&id);
        }

        // Phase 5.5: recent databases (local)
        if let Some(d) = app_state.0.databases.get().into_iter().find(|d| d.id == id) {
            write_recent_db(&d.id, &d.name);
        } else {
            // Fallback: keep at least the id.
            write_recent_db(&id, &id);
        }
    });

    // Phase 5 (non-paginated): load notes for current database.
    Effect::new(move |_| {
        load_notes_for_sv.with_value(|f| {
            f(db_id(), false);
        });
    });

    // UX: when user enters /db/:db_id, auto-open the first note.
    // This makes the main area show a note immediately and enables Pages highlight.
    Effect::new(move |_| {
        let id = db_id();
        if id.trim().is_empty() {
            return;
        }

        let p = pathname();
        if p != format!("/db/{}", id) {
            return;
        }

        if app_state.0.notes_loading.get() {
            return;
        }

        let mut notes = app_state
            .0
            .notes
            .get()
            .into_iter()
            .filter(|n| n.database_id == id)
            .collect::<Vec<_>>();

        if notes.is_empty() {
            return;
        }

        // Prefer most recently updated (lexicographic works for ISO-ish timestamps).
        notes.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        let first_id = notes[0].id.clone();

        // Use replace=true so browser Back goes to the previous page (e.g. Home),
        // instead of bouncing between /db/:db_id and /db/:db_id/note/:note_id.
        navigate.with_value(|nav| {
            nav(
                &format!("/db/{}/note/{}", id, first_id),
                leptos_router::NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            );
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

    let _on_open_rename = move |_: web_sys::MouseEvent| {
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

    let _on_open_delete = move |_: web_sys::MouseEvent| {
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

    let is_auto_opening_note = move || {
        let id = db_id();
        let p = pathname();
        if id.trim().is_empty() {
            return false;
        }
        if p != format!("/db/{}", id) {
            return false;
        }

        // If notes are loading, or we already have notes for this DB, we're about to auto-navigate.
        let has_notes = app_state
            .0
            .notes
            .get()
            .into_iter()
            .any(|n| n.database_id == id);

        app_state.0.notes_loading.get() || has_notes
    };

    view! {
        <Show
            when=move || !is_auto_opening_note()
            fallback=move || view! {
                <div class="flex h-[40vh] items-center justify-center">
                    <Spinner />
                </div>
            }
        >
            <div class="space-y-3">
                <div class="flex items-start justify-between gap-3">
                    <div class="space-y-1">
                        <h1 class="text-xl font-semibold">
                            {move || db().map(|d| d.name).unwrap_or_else(|| "Database".to_string())}
                        </h1>
                        <p class="text-xs text-muted-foreground">{move || format!("db_id: {}", db_id())}</p>
                    </div>

                    <div class="flex items-center gap-2"></div>
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
        </Show>
    }
}

#[component]
pub fn SearchPage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let query = use_query_map();

    let q = move || query.get().get("q").unwrap_or_default();
    let q_lower = move || q().trim().to_lowercase();

    let matched_dbs = move || {
        let q = q_lower();
        if q.is_empty() {
            return vec![];
        }
        app_state
            .0
            .databases
            .get()
            .into_iter()
            .filter(|d| d.name.to_lowercase().contains(&q))
            .collect::<Vec<_>>()
    };

    let matched_notes = move || {
        let q = q_lower();
        if q.is_empty() {
            return vec![];
        }
        let db_id = app_state.0.current_database_id.get().unwrap_or_default();
        if db_id.trim().is_empty() {
            return vec![];
        }

        app_state
            .0
            .notes
            .get()
            .into_iter()
            .filter(|n| n.database_id == db_id)
            .filter(|n| n.title.to_lowercase().contains(&q))
            .collect::<Vec<_>>()
    };

    view! {
        <div class="space-y-4">
            <div class="space-y-1">
                <h1 class="text-xl font-semibold">"Search"</h1>
                <p class="text-xs text-muted-foreground">{move || format!("q = {}", q())}</p>
            </div>

            <Show
                when=move || !q_lower().is_empty()
                fallback=|| view! {
                    <div class="rounded-md border border-border bg-muted p-4 text-sm text-muted-foreground">
                        "Type a query in the sidebar search box and press Enter."
                    </div>
                }
            >
                <div class="space-y-4">
                    <Card>
                        <CardHeader class="p-3">
                            <CardTitle class="text-sm">"Databases"</CardTitle>
                        </CardHeader>
                        <CardContent class="p-3 pt-0">
                            <Show
                                when=move || !matched_dbs().is_empty()
                                fallback=|| view! { <div class="text-sm text-muted-foreground">"No matching databases."</div> }
                            >
                                <div class="space-y-1">
                                    {move || {
                                        matched_dbs()
                                            .into_iter()
                                            .map(|db| {
                                                let id = db.id.clone();
                                                let id_href = id.clone();
                                                let name = db.name.clone();
                                                view! {
                                                    <a
                                                        href=format!("/db/{}", id_href)
                                                        class="block rounded-md border border-border bg-background px-3 py-2 transition-colors hover:bg-surface-hover"
                                                    >
                                                        <div class="truncate text-sm font-medium">{name}</div>
                                                        <div class="truncate text-xs text-muted-foreground">{id}</div>
                                                    </a>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </div>
                            </Show>
                        </CardContent>
                    </Card>

                    <div class="h-px w-full bg-border" />

                    <Card>
                        <CardHeader class="p-3">
                            <CardTitle class="text-sm">"Notes (current DB)"</CardTitle>
                        </CardHeader>
                        <CardContent class="p-3 pt-0">
                            <Show
                                when=move || !matched_notes().is_empty()
                                fallback=move || view! {
                                    <div class="text-sm text-muted-foreground">
                                        {move || {
                                            if app_state.0.current_database_id.get().is_none() {
                                                "Select a database first."
                                            } else {
                                                "No matching notes in current DB."
                                            }
                                        }}
                                    </div>
                                }
                            >
                                <div class="space-y-1">
                                    {move || {
                                        let db_id = app_state.0.current_database_id.get().unwrap_or_default();
                                        matched_notes()
                                            .into_iter()
                                            .map(|n| {
                                                let id = n.id.clone();
                                                let title = n.title.clone();
                                                view! {
                                                    <a
                                                        href=format!("/db/{}/note/{}", db_id, id)
                                                        class="block rounded-md border border-border bg-background px-3 py-2 transition-colors hover:bg-surface-hover"
                                                    >
                                                        <div class="truncate text-sm font-medium">{title}</div>
                                                    </a>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </div>
                            </Show>
                        </CardContent>
                    </Card>
                </div>
            </Show>
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
            username: "u".to_string(),
            password: "pass".to_string(),
            registration_code: "FA8E-AF6E-4578-9347".to_string(),
        };
        let v = serde_json::to_value(req).expect("should serialize");
        assert_eq!(v["email"], "u@example.com");
        assert_eq!(v["username"], "u");
        assert_eq!(v["registration-code"], "FA8E-AF6E-4578-9347");
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

    // NOTE: database list parsing is intentionally strict to the canonical contract.
    // The canonical database list shape is covered by `test_parse_database_list_response_legacy_shape`.

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

    // NOTE: note list parsing is intentionally strict to the canonical contract.
    // The canonical note list shape is covered by `test_parse_note_list_response_legacy_shape_note_list`.

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

    #[test]
    fn test_upsert_lru_by_key_dedup_and_order() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let out = upsert_lru_by_key(items, "b".to_string(), |x, y| x == y, 10);
        assert_eq!(out, vec!["b", "a", "c"]);
    }

    #[test]
    fn test_upsert_lru_by_key_truncate() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let out = upsert_lru_by_key(items, "d".to_string(), |x, y| x == y, 3);
        assert_eq!(out, vec!["d", "a", "b"]);
    }

    #[test]
    fn test_recent_structs_serde_roundtrip() {
        let db = RecentDb {
            id: "db1".to_string(),
            name: "My DB".to_string(),
            last_opened_ms: 123,
        };
        let note = RecentNote {
            db_id: "db1".to_string(),
            note_id: "n1".to_string(),
            title: "T".to_string(),
            last_opened_ms: 456,
        };

        let db_json = serde_json::to_string(&db).unwrap();
        let db2: RecentDb = serde_json::from_str(&db_json).unwrap();
        assert_eq!(db, db2);

        let note_json = serde_json::to_string(&note).unwrap();
        let note2: RecentNote = serde_json::from_str(&note_json).unwrap();
        assert_eq!(note, note2);
    }
}
