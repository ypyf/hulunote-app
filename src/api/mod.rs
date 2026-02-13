use crate::models::{AccountInfo, Database, Nav, Note};
use crate::storage::{TOKEN_KEY, USER_KEY};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ApiErrorKind {
    Unauthorized,
    Network,
    Http,
    Parse,
    Other,
}

#[derive(Clone, Debug)]
pub(crate) struct ApiError {
    pub kind: ApiErrorKind,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl ApiError {
    fn network(e: reqwest::Error) -> Self {
        Self {
            kind: ApiErrorKind::Network,
            message: e.to_string(),
        }
    }

    fn parse(e: impl std::fmt::Display) -> Self {
        Self {
            kind: ApiErrorKind::Parse,
            message: e.to_string(),
        }
    }

    fn unauthorized() -> Self {
        Self {
            kind: ApiErrorKind::Unauthorized,
            message: "Unauthorized".to_string(),
        }
    }

    fn http(status: reqwest::StatusCode, body: String, ctx: &str) -> Self {
        Self {
            kind: ApiErrorKind::Http,
            message: format!("{ctx} ({status}): {body}"),
        }
    }

    fn other(msg: impl Into<String>) -> Self {
        Self {
            kind: ApiErrorKind::Other,
            message: msg.into(),
        }
    }
}

pub(crate) type ApiResult<T> = Result<T, ApiError>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct EnvConfig {
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct LoginResponse {
    pub token: String,
    pub hulunote: AccountInfo,
    pub region: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct CreateDatabaseRequest {
    // hulunote-rust expects kebab-case keys.
    #[serde(rename = "database-name")]
    pub database_name: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct UpdateDatabaseRequest {
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
pub(crate) struct DeleteDatabaseRequest {
    #[serde(rename = "database-id", skip_serializing_if = "Option::is_none")]
    pub database_id: Option<String>,
    #[serde(rename = "database-name", skip_serializing_if = "Option::is_none")]
    pub database_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct CreateNoteRequest {
    #[serde(rename = "database-id")]
    pub database_id: String,
    pub title: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct GetNoteListRequest {
    pub database_id: String,
    pub page: i32,
    pub page_size: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct GetNoteNavsRequest {
    #[serde(rename = "note-id")]
    pub note_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct CreateOrUpdateNavRequest {
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SignupRequest {
    pub email: String,
    pub username: String,
    pub password: String,
    pub registration_code: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct SignupResponse {
    pub token: String,
    pub hulunote: AccountInfo,
    pub database: Option<String>,
    pub region: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ApiClient {
    pub(crate) base_url: String,
    pub(crate) token: Option<String>,
}

impl ApiClient {
    #[allow(dead_code)]
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
    pub(crate) fn get_auth_token(&self) -> Option<String> {
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

    pub(crate) fn parse_database_list_response(data: serde_json::Value) -> Vec<Database> {
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

    pub(crate) fn parse_note_list_response(data: serde_json::Value) -> Vec<Note> {
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

    pub async fn get_all_note_list(&self, database_id: &str) -> ApiResult<Vec<Note>> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/get-all-note-list", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        // hulunote-rust handler expects kebab-case: `database-id`
        let res = req
            .json(&serde_json::json!({ "database-id": database_id }))
            .send()
            .await
            .map_err(ApiError::network)?;

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(ApiError::parse)?;
            Ok(Self::parse_note_list_response(data))
        } else if res.status().as_u16() == 401 {
            Err(ApiError::unauthorized())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(ApiError::http(status, body, "Failed to get notes"))
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
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Create database failed ({status}): {body}"))
        }
    }

    pub async fn rename_database(&self, database_id: &str, name: &str) -> Result<(), String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/update-database", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&UpdateDatabaseRequest {
                database_id: Some(database_id.to_string()),
                id: None,
                db_name: Some(name.to_string()),
                is_public: None,
                is_default: None,
                is_delete: None,
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            Ok(())
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
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
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(format!("Delete database failed ({status}): {body}"))
        }
    }

    pub async fn create_note(&self, database_id: &str, title: &str) -> Result<Note, String> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/new-note", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&CreateNoteRequest {
                database_id: database_id.to_string(),
                title: title.to_string(),
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            let id = data
                .get("note")
                .and_then(|n| n.get("hulunote-notes/id"))
                .or_else(|| data.get("note-id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            Ok(Note {
                id: id.to_string(),
                database_id: database_id.to_string(),
                title: title.to_string(),
                content: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
            })
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

        let res = req
            .json(&serde_json::json!({ "note-id": note_id, "title": title }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

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

    pub async fn get_note_navs(&self, note_id: &str) -> ApiResult<Vec<Nav>> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/get-note-navs", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&GetNoteNavsRequest {
                note_id: note_id.to_string(),
            })
            .send()
            .await
            .map_err(ApiError::network)?;

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(ApiError::parse)?;
            Ok(Self::parse_nav_list_response(data))
        } else if res.status().as_u16() == 401 {
            Err(ApiError::unauthorized())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(ApiError::http(status, body, "Failed to get navs"))
        }
    }

    pub async fn get_all_navs(&self, database_id: &str) -> ApiResult<Vec<Nav>> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/get-all-navs", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&serde_json::json!({ "database-id": database_id }))
            .send()
            .await
            .map_err(ApiError::network)?;

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(ApiError::parse)?;
            Ok(Self::parse_nav_list_response(data))
        } else if res.status().as_u16() == 401 {
            Err(ApiError::unauthorized())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(ApiError::http(status, body, "Failed to get navs"))
        }
    }

    pub async fn upsert_nav(
        &self,
        req_body: CreateOrUpdateNavRequest,
    ) -> ApiResult<serde_json::Value> {
        let client = reqwest::Client::new();
        let req = client.post(format!("{}/hulunote/create-or-update-nav", self.base_url));
        let req = Self::with_auth_headers(req, self.get_auth_token());

        let res = req
            .json(&req_body)
            .send()
            .await
            .map_err(ApiError::network)?;

        if res.status().is_success() {
            res.json().await.map_err(ApiError::parse)
        } else if res.status().as_u16() == 401 {
            Err(ApiError::unauthorized())
        } else {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(ApiError::http(status, body, "Upsert nav failed"))
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

    pub(crate) fn parse_nav_list_response(data: serde_json::Value) -> Vec<Nav> {
        let list = data
            .get("nav-list")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut out: Vec<Nav> = Vec::with_capacity(list.len());
        for item in list {
            // Preferred: canonical contract uses non-namespaced kebab-case keys.
            // We also accept namespaced variants defensively.
            if let Ok(nav) = serde_json::from_value::<Nav>(item.clone()) {
                out.push(nav);
                continue;
            }

            let get_s = |k: &str| item.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
            let get_f = |k: &str| item.get(k).and_then(|v| v.as_f64());
            let get_b = |k: &str| item.get(k).and_then(|v| v.as_bool());

            let id = get_s("id")
                .or_else(|| get_s("hulunote-navs/id"))
                .unwrap_or_default();

            let note_id = get_s("note-id")
                .or_else(|| get_s("hulunote-navs/note-id"))
                .unwrap_or_default();

            let parid = get_s("parid")
                .or_else(|| get_s("hulunote-navs/parid"))
                .unwrap_or_default();

            let same_deep_order = get_f("same-deep-order")
                .or_else(|| get_f("hulunote-navs/same-deep-order"))
                .unwrap_or(0.0) as f32;

            let content = get_s("content")
                .or_else(|| get_s("hulunote-navs/content"))
                .unwrap_or_default();

            let is_display = get_b("is-display")
                .or_else(|| get_b("hulunote-navs/is-display"))
                .unwrap_or(true);

            let is_delete = get_b("is-delete")
                .or_else(|| get_b("hulunote-navs/is-delete"))
                .unwrap_or(false);

            if !id.trim().is_empty() && !note_id.trim().is_empty() {
                let properties = get_s("properties")
                    .or_else(|| get_s("hulunote-navs/properties"))
                    .filter(|s| !s.trim().is_empty());

                out.push(Nav {
                    id,
                    note_id,
                    parid,
                    same_deep_order,
                    content,
                    is_display,
                    is_delete,
                    properties,
                });
            }
        }

        out
    }
}
