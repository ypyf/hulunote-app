use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::path;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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
    pub username: Option<String>,
    pub password: String,
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

    fn get_auth_header(&self) -> Option<String> {
        self.token.as_ref().map(|t| format!("Bearer {}", t))
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
            Err("Login failed".to_string())
        }
    }

    async fn request_database_list(
        base_url: &str,
        auth_header: Option<String>,
    ) -> Result<reqwest::Response, String> {
        let client = reqwest::Client::new();
        let mut req = client.post(format!("{}/hulunote/get-database-list", base_url));
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
        let res = Self::request_database_list(&self.base_url, self.get_auth_header()).await?;

        // Backend (hulunote-rust) does not provide a refresh-token endpoint.
        // If token is invalid/expired, caller should force re-login.

        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            Ok(serde_json::from_value(data["databases"].clone()).map_err(|e| e.to_string())?)
        } else if res.status().as_u16() == 401 {
            Err("Unauthorized".to_string())
        } else {
            Err("Failed to get databases".to_string())
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
                username: if username.trim().is_empty() {
                    None
                } else {
                    Some(username.to_string())
                },
                password: password.to_string(),
                registration_code: registration_code.to_string(),
            })
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status().is_success() {
            res.json().await.map_err(|e| e.to_string())
        } else {
            Err("Signup failed".to_string())
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
    pub databases: RwSignal<Vec<Database>>,
}

const TOKEN_KEY: &str = "hulunote_token";
const USER_KEY: &str = "hulunote_user";

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
        Self {
            api_client: RwSignal::new(stored_client),
            current_user: RwSignal::new(stored_user),
            databases: RwSignal::new(vec![]),
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
    let registration_code: RwSignal<String> = RwSignal::new(String::new());
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let loading: RwSignal<bool> = RwSignal::new(false);
    let success: RwSignal<bool> = RwSignal::new(false);

    let app_state = expect_context::<AppContext>();

    let on_submit = move |_| {
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

    let registration_code_input = move |e: web_sys::Event| {
        if let Some(target) = e.target() {
            if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                registration_code.set(input.value());
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
                                    class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                                    on:input=confirm_password_input
                                />
                            </div>
                            <div>
                                <input
                                    type="text"
                                    required
                                    placeholder="Registration code (required)"
                                    class="appearance-none rounded-none relative block w-full px-3 py-2 border border-gray-300 placeholder-gray-500 text-gray-900 rounded-b-md focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm"
                                    on:input=registration_code_input
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
pub fn RootPage() -> impl IntoView {
    let app_state = expect_context::<AppContext>();
    let is_authenticated = move || app_state.0.api_client.get().is_authenticated();

    view! {
        <Show when=is_authenticated fallback=move || view! { <LoginPage /> }>
            <HomePage />
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
            <Routes fallback=|| view! { <div class="p-4 text-gray-600">"Not found"</div> }>
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
        };
        let v = serde_json::to_value(req).expect("should serialize");
        assert_eq!(v["email"], "u@example.com");
        assert_eq!(v["registration_code"], "FA8E-AF6E-4578-9347");
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
}
