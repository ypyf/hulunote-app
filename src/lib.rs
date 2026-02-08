use wasm_bindgen::prelude::*;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_location;
use serde::{Deserialize, Serialize};

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

fn get_api_url() -> String {
    EnvConfig::new().api_url
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub id: String,
    pub email: String,
    pub username: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LoginResponse {
    pub token: String,
    pub refresh_token: Option<String>,
    pub user: User,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RefreshTokenResponse {
    pub token: String,
    pub refresh_token: Option<String>,
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
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignupResponse {
    pub message: String,
    pub user: Option<User>,
}

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    token: Option<String>,
    refresh_token: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            token: None,
            refresh_token: None,
        }
    }

    pub fn load_from_storage() -> Self {
        let base_url = get_api_url();
        let token = leptos::web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(TOKEN_KEY).ok().flatten());

        let refresh_token = leptos::web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(REFRESH_TOKEN_KEY).ok().flatten());

        Self {
            base_url,
            token,
            refresh_token,
        }
    }

    pub fn save_to_storage(&self) {
        if let Some(storage) = leptos::web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
        {
            if let Some(token) = &self.token {
                let _ = storage.set_item(TOKEN_KEY, token);
            }
            if let Some(refresh_token) = &self.refresh_token {
                let _ = storage.set_item(REFRESH_TOKEN_KEY, refresh_token);
            }
        }
    }

    pub fn clear_storage() {
        if let Some(storage) = leptos::web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
        {
            let _ = storage.remove_item(TOKEN_KEY);
            let _ = storage.remove_item(REFRESH_TOKEN_KEY);
            let _ = storage.remove_item(USER_KEY);
        }
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    pub fn set_refresh_token(&mut self, refresh_token: Option<String>) {
        self.refresh_token = refresh_token;
    }

    pub fn get_token(&self) -> Option<&String> {
        self.token.as_ref()
    }

    fn get_auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| format!("Bearer {}", t))
    }

    pub async fn refresh_token(&mut self) -> Result<bool, String> {
        let refresh_token = match &self.refresh_token {
            Some(token) => token.clone(),
            None => return Err("No refresh token available".to_string()),
        };

        let client = reqwest::Client::new();
        let res = client
            .post(&format!("{}/login/refresh", self.base_url))
            .json(&RefreshTokenRequest {
                refresh_token,
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            let response: RefreshTokenResponse = res.json().await.map_err(|e| e.to_string())?;
            self.token = Some(response.token);
            self.refresh_token = response.refresh_token;
            self.save_to_storage();
            Ok(true)
        } else {
            Err("Token refresh failed".to_string())
        }
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<LoginResponse, String> {
        let client = reqwest::Client::new();
        let res = client
            .post(&format!("{}/login/web-login", self.base_url))
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
            Err(format!("Login failed"))
        }
    }

    async fn request_database_list(
        base_url: &str,
        auth_header: Option<String>,
    ) -> Result<reqwest::Response, String> {
        let client = reqwest::Client::new();
        let mut req = client.post(&format!("{}/hulunote/get-database-list", base_url));
        if let Some(header) = auth_header {
            req = req.header("Authorization", header);
        }
        req.json(&serde_json::json!({}))
            .send()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_database_list(&mut self) -> Result<Vec<Database>, String> {
        // First try with current token
        let mut res = Self::request_database_list(&self.base_url, self.get_auth_header()).await?;

        // If unauthorized, attempt refresh once then retry
        if res.status().as_u16() == 401 {
            let refreshed = self.refresh_token().await.unwrap_or(false);
            if refreshed {
                res = Self::request_database_list(&self.base_url, self.get_auth_header()).await?;
            }
        }

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            Ok(serde_json::from_value(data["databases"].clone()).map_err(|e| e.to_string())?)
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            Err("Failed to get databases".to_string())
        }
    }

    pub async fn signup(&self, email: &str, username: &str, password: &str) -> Result<SignupResponse, String> {
        let client = reqwest::Client::new();
        let res = client
            .post(&format!("{}/login/web-signup", self.base_url))
            .json(&SignupRequest {
                email: email.to_string(),
                username: username.to_string(),
                password: password.to_string(),
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            res.json().await.map_err(|e| e.to_string())
        } else {
            Err(format!("Signup failed"))
        }
    }

    pub fn logout(&mut self) {
        self.token = None;
        self.refresh_token = None;
        Self::clear_storage();
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
}

#[derive(Clone)]
pub struct AppState {
    pub api_client: RwSignal<ApiClient>,
    pub current_user: RwSignal<Option<User>>,
    pub databases: RwSignal<Vec<Database>>,
}

const TOKEN_KEY: &str = "hulunote_token";
const REFRESH_TOKEN_KEY: &str = "hulunote_refresh_token";
const USER_KEY: &str = "hulunote_user";

fn save_user_to_storage(user: &User) {
    if let Ok(json) = serde_json::to_string(user) {
        if let Some(storage) = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
        {
            let _ = storage.set_item(USER_KEY, &json);
        }
    }
}

fn load_user_from_storage() -> Option<User> {
    if let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
    {
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
        Self {
            api_client: RwSignal::new(stored_client),
            current_user: RwSignal::new(stored_user),
            databases: RwSignal::new(vec![]),
        }
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

    let on_submit = move |_| {
        let email_val = email.get();
        let password_val = password.get();
        let mut api_client = app_state.0.api_client.get_untracked();

        loading.set(true);
        error.set(None);

        spawn_local(async move {
            match api_client.login(&email_val, &password_val).await {
                Ok(response) => {
                    api_client.set_token(response.token);
                    api_client.set_refresh_token(response.refresh_token);
                    api_client.save_to_storage();
                    save_user_to_storage(&response.user);
                    app_state.0.api_client.set(api_client);
                    app_state.0.current_user.set(Some(response.user));
                    let _ = window().location().set_href("/");
                }
                Err(e) => {
                    error.set(Some(e));
                }
            }
            loading.set(false);
        });
    };

    let email_input = move |e: web_sys::Event| {
        if let Some(target) = e.target() {
            if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                email.set(input.value());
            }
        }
    };

    let password_input = move |e: web_sys::Event| {
        if let Some(target) = e.target() {
            if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                password.set(input.value());
            }
        }
    };

    view! {
        <div class="min-h-screen flex items-center justify-center bg-gray-50 py-12 px-4">
            <div class="max-w-md w-full space-y-8">
                    <div>
                        <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900">
                            "Sign in to Hulunote"
                        </h2>
                        <p class="mt-2 text-center text-sm text-gray-600">
                            "Or "
                            <a href="/signup" class="font-medium text-indigo-600 hover:text-indigo-500">
                                "create a new account"
                            </a>
                        </p>
                    </div>
                <form class="mt-8 space-y-6" on:submit=on_submit>
                    <div class="rounded-md shadow-sm -space-y-px">
                        <div>
                            <input
                                type="email"
                                required
                                placeholder="Email address"
                                class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-t-md focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                                on:input=email_input
                            />
                        </div>
                        <div>
                            <input
                                type="password"
                                required
                                placeholder="Password"
                                class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-b-md focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                                on:input=password_input
                            />
                        </div>
                    </div>

                    {move || error.get().map(|e| view! { <div class="text-red-500 text-sm text-center">{e}</div> })}

                    <div>
                        <button
                            type="submit"
                            disabled=loading.get()
                            class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-white bg-indigo-600 hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500 disabled:opacity-50"
                        >
                            {move || if loading.get() { "Signing in..." } else { "Sign in" }}
                        </button>
                    </div>
                </form>
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
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let loading: RwSignal<bool> = RwSignal::new(false);
    let success: RwSignal<bool> = RwSignal::new(false);

    let app_state = expect_context::<AppContext>();

    let on_submit = move |_| {
        let email_val = email.get();
        let username_val = username.get();
        let password_val = password.get();
        let confirm_password_val = confirm_password.get();
        let api_client = app_state.0.api_client.get_untracked();

        if password_val != confirm_password_val {
            error.set(Some("Passwords do not match".to_string()));
            return;
        }

        if password_val.len() < 6 {
            error.set(Some("Password must be at least 6 characters".to_string()));
            return;
        }

        loading.set(true);
        error.set(None);

        spawn_local(async move {
            match api_client.signup(&email_val, &username_val, &password_val).await {
                Ok(_response) => {
                    success.set(true);
                }
                Err(e) => {
                    error.set(Some(e));
                }
            }
            loading.set(false);
        });
    };

    let email_input = move |e: web_sys::Event| {
        if let Some(target) = e.target() {
            if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                email.set(input.value());
            }
        }
    };

    let username_input = move |e: web_sys::Event| {
        if let Some(target) = e.target() {
            if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                username.set(input.value());
            }
        }
    };

    let password_input = move |e: web_sys::Event| {
        if let Some(target) = e.target() {
            if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                password.set(input.value());
            }
        }
    };

    let confirm_password_input = move |e: web_sys::Event| {
        if let Some(target) = e.target() {
            if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                confirm_password.set(input.value());
            }
        }
    };

    view! {
        <div class="min-h-screen flex items-center justify-center bg-gray-50 py-12 px-4">
            <div class="max-w-md w-full space-y-8">
                <div>
                    <h2 class="mt-6 text-center text-3xl font-extrabold text-gray-900">
                        "Create your account"
                    </h2>
                    <p class="mt-2 text-center text-sm text-gray-600">
                        "Or "
                        <a href="/login" class="font-medium text-indigo-600 hover:text-indigo-500">
                            "sign in to existing account"
                        </a>
                    </p>
                </div>
                <Show when=move || !success.get() fallback=move || view! {
                    <div class="text-center">
                        <div class="rounded-md bg-green-50 p-4 mb-4">
                            <div class="flex">
                                <div class="flex-shrink-0">
                                    <svg class="h-5 w-5 text-green-400" viewBox="0 0 20 20" fill="currentColor">
                                        <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
                                    </svg>
                                </div>
                                <div class="ml-3">
                                    <p class="text-sm font-medium text-green-800">
                                        "Account created successfully! Redirecting to login..."
                                    </p>
                                </div>
                            </div>
                        </div>
                        <a href="/login" class="font-medium text-indigo-600 hover:text-indigo-500">
                            "Click here to sign in"
                        </a>
                    </div>
                }>
                    <form class="mt-8 space-y-6" on:submit=on_submit>
                        <div class="rounded-md shadow-sm -space-y-px">
                            <div>
                                <input
                                    type="text"
                                    required
                                    placeholder="Username"
                                    class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-t-md focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                                    on:input=username_input
                                />
                            </div>
                            <div>
                                <input
                                    type="email"
                                    required
                                    placeholder="Email address"
                                    class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                                    on:input=email_input
                                />
                            </div>
                            <div>
                                <input
                                    type="password"
                                    required
                                    placeholder="Password (min 6 characters)"
                                    class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                                    on:input=password_input
                                />
                            </div>
                            <div>
                                <input
                                    type="password"
                                    required
                                    placeholder="Confirm password"
                                    class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-b-md focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                                    on:input=confirm_password_input
                                />
                            </div>
                        </div>

                        {move || error.get().map(|e| view! { <div class="text-red-500 text-sm text-center">{e}</div> })}

                        <div>
                            <button
                                type="submit"
                                disabled=loading.get()
                                class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-white bg-indigo-600 hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500 disabled:opacity-50"
                            >
                                {move || if loading.get() { "Creating account..." } else { "Create account" }}
                            </button>
                        </div>
                    </form>
                </Show>
            </div>
        </div>
    }
}

#[component]
pub fn HomePage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let databases = app_state.0.databases;

    let loading: RwSignal<bool> = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);

    // Load databases once we land on the home page.
    // If token is invalid/expired, ApiClient::get_database_list will try refresh once.
    // If still unauthorized, we force logout + redirect to /login.
    let load_databases = move || {
        let mut api_client = app_state.0.api_client.get_untracked();
        loading.set(true);
        error.set(None);

        spawn_local(async move {
            match api_client.get_database_list().await {
                Ok(dbs) => {
                    app_state.0.databases.set(dbs);
                    app_state.0.api_client.set(api_client);
                }
                Err(e) => {
                    if e == "Unauthorized" {
                        // Session is not valid anymore.
                        api_client.logout();
                        app_state.0.api_client.set(api_client);
                        app_state.0.current_user.set(None);
                        let _ = window().location().set_href("/login");
                    } else {
                        error.set(Some(e));
                        app_state.0.api_client.set(api_client);
                    }
                }
            }
            loading.set(false);
        });
    };

    // Trigger on mount
    Effect::new(move |_| {
        load_databases();
    });

    view! {
        <div class="min-h-screen bg-gray-50">
            <nav class="bg-white shadow-sm">
                <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
                    <div class="flex justify-between h-16">
                        <div class="flex">
                            <div class="flex-shrink-0 flex items-center">
                                <h1 class="text-xl font-bold text-gray-900">"Hulunote"</h1>
                            </div>
                        </div>
                        <div class="flex items-center gap-3">
                            <button
                                class="text-gray-500 hover:text-gray-700"
                                on:click=move |_| load_databases()
                                disabled=loading.get()
                            >
                                {move || if loading.get() { "Refreshing..." } else { "Refresh" }}
                            </button>
                            <button class="text-gray-500 hover:text-gray-700">"Settings"</button>
                        </div>
                    </div>
                </div>
            </nav>

            <div class="max-w-7xl mx-auto py-6 sm:px-6 lg:px-8">
                <div class="px-4 py-6 sm:px-0 space-y-4">
                    {move || error.get().map(|e| view!{
                        <div class="rounded-md bg-red-50 p-4 text-sm text-red-700">{e}</div>
                    })}

                    <div class="border border-gray-200 bg-white rounded-lg p-4">
                        <div class="flex items-center justify-between">
                            <h2 class="text-lg font-semibold text-gray-900">"Databases"</h2>
                            <span class="text-sm text-gray-500">{move || format!("{}", databases.get().len())}</span>
                        </div>

                        <div class="mt-3">
                            <Show
                                when=move || !databases.get().is_empty()
                                fallback=move || view! {
                                    <p class="text-gray-500">
                                        {move || if loading.get() {
                                            "Loading databases..."
                                        } else {
                                            "No databases yet. Create your first database to get started."
                                        }}
                                    </p>
                                }
                            >
                                <ul class="space-y-2">
                                    {move || {
                                        databases
                                            .get()
                                            .into_iter()
                                            .map(|db| view! {
                                                <li class="border border-gray-100 rounded p-2">
                                                    <div class="font-medium text-gray-900">{db.name}</div>
                                                    <div class="text-sm text-gray-500">{db.description}</div>
                                                </li>
                                            })
                                            .collect_view()
                                    }}
                                </ul>
                            </Show>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_context(AppContext(AppState::new()));

    let location = use_location();
    let pathname = move || location.pathname.get();

    let app_state = expect_context::<AppContext>();

    // IMPORTANT: use reactive reads (get), not get_untracked, so auth changes rerender.
    let is_authenticated = move || app_state.0.api_client.get().is_authenticated();

    // Simple route protection: if user is not authenticated and tries to access any
    // non-auth page, redirect to /login.
    Effect::new(move |_| {
        let path = pathname();
        let authed = is_authenticated();
        let is_auth_page = path == "/login" || path == "/signup";

        if !authed && !is_auth_page {
            let _ = window().location().set_href("/login");
        }

        // Optional: if already authenticated and visits /login, send them home
        if authed && path == "/login" {
            let _ = window().location().set_href("/");
        }
    });

    view! {
        <Show
            when=move || pathname() == "/login"
            fallback=move || view! {
                <Show
                    when=move || pathname() == "/signup"
                    fallback=move || view! {
                        <Show
                            when=is_authenticated
                            fallback=move || view! { <LoginPage /> }
                        >
                            <HomePage />
                        </Show>
                    }
                >
                    <RegistrationPage />
                </Show>
            }
        >
            <LoginPage />
        </Show>
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_client_new() {
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert_eq!(client.base_url, "http://localhost:6689");
        assert!(client.token.is_none());
        assert!(client.refresh_token.is_none());
    }

    #[test]
    fn test_api_client_set_token() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_token("test-token".to_string());
        assert_eq!(client.token, Some("test-token".to_string()));
    }

    #[test]
    fn test_api_client_get_auth_header_without_token() {
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert!(client.get_auth_header().is_none());
    }

    #[test]
    fn test_api_client_get_auth_header_with_token() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_token("my-jwt-token".to_string());
        let header = client.get_auth_header().expect("Should have auth header");
        assert_eq!(header, "Bearer my-jwt-token");
    }

    #[test]
    fn test_api_client_set_refresh_token() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_refresh_token(Some("my-refresh-token".to_string()));
        assert_eq!(client.refresh_token, Some("my-refresh-token".to_string()));
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
}
