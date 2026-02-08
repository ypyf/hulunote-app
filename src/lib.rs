mod components;

use crate::components::ui::{
    Alert, AlertDescription, Button, ButtonSize, ButtonVariant, Card, CardContent, CardDescription,
    CardHeader, CardItem, CardList, CardTitle, Input, Label, Spinner,
};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use serde::{Deserialize, Serialize};

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
    pub name: String,
    pub description: String,
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

    async fn request_database_list(
        base_url: &str,
        token: Option<String>,
    ) -> Result<reqwest::Response, String> {
        let client = reqwest::Client::new();
        let mut req = client.post(format!("{}/hulunote/get-database-list", base_url));

        // Match the legacy client contract.
        if let Some(token) = token {
            req = req.header("X-FUNCTOR-API-TOKEN", token);
        }

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

    /// Current database selection (drives routing in later phases).
    pub current_database_id: RwSignal<Option<String>>,

    /// Global UI state.
    pub sidebar_collapsed: RwSignal<bool>,
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
            current_database_id: RwSignal::new(current_database_id),
            sidebar_collapsed: RwSignal::new(sidebar_collapsed),
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

                <div class="rounded-md border border-border bg-muted px-5 py-5 shadow-sm">
                    <div class="mb-4 space-y-1">
                        <h1 class="text-lg font-medium text-foreground">"Log in"</h1>
                        <p class="text-xs text-muted-foreground">"Use your email and password to continue."</p>
                    </div>

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
                </div>
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

                <div class="rounded-md border border-border bg-muted px-5 py-5 shadow-sm">
                    <div class="mb-4 space-y-1">
                        <h1 class="text-lg font-medium text-foreground">"Create account"</h1>
                        <p class="text-xs text-muted-foreground">"A registration code is required."</p>
                    </div>

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
                </div>
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

            <div class="rounded-md border border-border bg-muted p-4 text-sm text-muted-foreground">
                "Phase 3: Layout & Navigation. Main content will become note list/editor in later phases."
            </div>
        </div>
    }
}

#[component]
pub fn AppLayout(children: Children) -> impl IntoView {
    let app_state = expect_context::<AppContext>();

    let databases = app_state.0.databases;
    let current_db_id = app_state.0.current_database_id;
    let sidebar_collapsed = app_state.0.sidebar_collapsed;

    let db_loading: RwSignal<bool> = RwSignal::new(false);
    let db_error: RwSignal<Option<String>> = RwSignal::new(None);

    let sidebar_width_class = move || if sidebar_collapsed.get() { "w-14" } else { "w-64" };

    let persist_sidebar = move || {
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item(
                SIDEBAR_COLLAPSED_KEY,
                if sidebar_collapsed.get() { "1" } else { "0" },
            );
        }
    };

    let persist_current_db = move || {
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let v = current_db_id.get().unwrap_or_default();
            let _ = storage.set_item(CURRENT_DB_KEY, &v);
        }
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
                current_db_id.set(Some(first.id.clone()));
                persist_current_db();
            }
        }
    });

    let on_toggle_sidebar = move |_| {
        sidebar_collapsed.update(|v| *v = !*v);
        persist_sidebar();
    };

    let on_select_db = move |id: String| {
        current_db_id.set(Some(id));
        persist_current_db();
    };

    let on_logout = move |_| {
        let mut api_client = app_state.0.api_client.get_untracked();
        api_client.logout();
        app_state.0.api_client.set(api_client);
        app_state.0.current_user.set(None);
        app_state.0.databases.set(vec![]);
        app_state.0.current_database_id.set(None);
        let _ = window().location().set_href("/login");
    };

    view! {
        <div class="min-h-screen bg-background text-foreground">
            <div class="mx-auto flex min-h-screen w-full max-w-[1080px] gap-4 px-4 py-6">
                <aside class=move || format!("{} shrink-0", sidebar_width_class())>
                    <div class="sticky top-6 space-y-4">
                        <div class="flex items-center justify-between">
                            <a href="/" class="text-sm font-medium text-foreground">
                                <Show when=move || !sidebar_collapsed.get() fallback=|| view! { "H" }>
                                    "Hulunote"
                                </Show>
                            </a>

                            <button
                                class="rounded-md border border-border bg-muted px-2 py-1 text-[11px] text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                                on:click=on_toggle_sidebar
                                title="Toggle sidebar"
                            >
                                {move || if sidebar_collapsed.get() { ">" } else { "<" }}
                            </button>
                        </div>

                        <Show
                            when=move || !sidebar_collapsed.get()
                            fallback=|| view! {
                                <div class="rounded-md border border-border bg-muted p-3 text-[11px] text-muted-foreground">
                                    "Sidebar collapsed"
                                </div>
                            }
                        >
                            <div class="rounded-md border border-border bg-muted p-3">
                                <div class="flex items-center justify-between">
                                    <div class="text-[11px] font-medium text-muted-foreground">"Databases"</div>
                                    <button
                                        class="text-[11px] text-muted-foreground hover:text-foreground"
                                        on:click=move |_| load_databases()
                                        title="Refresh"
                                    >
                                        "↻"
                                    </button>
                                </div>

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
                                                    let class = if is_selected {
                                                        "w-full rounded-md bg-accent px-2 py-1 text-left text-sm text-accent-foreground"
                                                    } else {
                                                        "w-full rounded-md px-2 py-1 text-left text-sm hover:bg-accent hover:text-accent-foreground"
                                                    };

                                                    let id = db.id.clone();
                                                    view! {
                                                        <button class=class on:click=move |_| on_select_db(id.clone())>
                                                            {db.name}
                                                        </button>
                                                    }
                                                })
                                                .collect_view()
                                        }}
                                    </Show>
                                </div>
                            </div>

                            <div class="rounded-md border border-border bg-muted p-3">
                                <div class="text-[11px] font-medium text-muted-foreground">"Account"</div>
                                <div class="mt-2">
                                    <Button
                                        variant=ButtonVariant::Outline
                                        size=ButtonSize::Sm
                                        on:click=on_logout
                                        class="w-full"
                                    >
                                        "Sign out"
                                    </Button>
                                </div>
                            </div>
                        </Show>
                    </div>
                </aside>

                <main class="min-w-0 flex-1">{children()}</main>
            </div>
        </div>
    }
}

#[component]
pub fn RootPage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let is_authenticated = move || app_state.0.api_client.get().is_authenticated();

    view! {
        <Show when=is_authenticated fallback=move || view! { <LoginPage /> }>
            <AppLayout>
                <HomePage />
            </AppLayout>
        </Show>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_context(AppContext(AppState::new()));

    // IMPORTANT:
    // - Leptos CSR requires the `csr` feature on `leptos`.
    // - `use_location()`/router hooks require a <Router> context.
    view! {
        <Router>
            <Routes fallback=|| view! { <div class="px-4 py-8 text-xs text-muted-foreground">"Not found"</div> }>
                <Route path=path!("login") view=LoginPage />
                <Route path=path!("signup") view=RegistrationPage />
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
}
