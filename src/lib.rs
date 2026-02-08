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

        if let Some(window) = web_sys::window() {
            if let Some(env) = window.get("ENV") {
                if !env.is_undefined() && env.is_object() {
                    match js_sys::Reflect::get(&env, &"api_url".into()) {
                        Ok(api_url) => {
                            if let Some(url_str) = api_url.as_string() {
                                return Self { api_url: url_str };
                            }
                        }
                        Err(_) => {}
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

    pub async fn get_database_list(&self) -> Result<Vec<Database>, String> {
        let client = reqwest::Client::new();
        let mut req = client.post(&format!("{}/hulunote/get-database-list", self.base_url));
        if let Some(header) = self.get_auth_header() {
            req = req.header("Authorization", header);
        }
        let res = req.json(&serde_json::json!({})).send().await.map_err(|e| e.to_string())?;
        if res.status().is_success() {
            let data: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;
            Ok(serde_json::from_value(data["databases"].clone()).map_err(|e| e.to_string())?)
        } else {
            Err(format!("Failed to get databases"))
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
                        <div class="flex items-center">
                            <button class="text-gray-500 hover:text-gray-700">"Settings"</button>
                        </div>
                    </div>
                </div>
            </nav>

            <div class="max-w-7xl mx-auto py-6 sm:px-6 lg:px-8">
                <div class="px-4 py-6 sm:px-0">
                    <div class="border-4 border-dashed border-gray-200 rounded-lg h-96 flex items-center justify-center">
                        <p class="text-gray-500">
                            {move || if databases.get().is_empty() {
                                "No databases yet. Create your first database to get started."
                            } else {
                                "Select a database to view notes."
                            }}
                        </p>
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
    let is_authenticated = move || app_state.0.api_client.get_untracked().is_authenticated();

    view! {
        <Show when=move || pathname() == "/login" fallback=move || view! {
            <Show when=move || pathname() == "/signup" fallback=move || view! {
                <Show when=is_authenticated fallback=move || view! {
                    <HomePage />
                }>
                    <HomePage />
                </Show>
            }>
                <RegistrationPage />
            </Show>
        }>
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
    use serde_json::json;

    #[test]
    fn test_user_serialization() {
        let user = User {
            id: "test-id-123".to_string(),
            email: "test@example.com".to_string(),
            username: "testuser".to_string(),
        };

        let serialized = serde_json::to_string(&user).expect("Failed to serialize User");
        assert!(serialized.contains("test@example.com"));
        assert!(serialized.contains("testuser"));

        let deserialized: User = serde_json::from_str(&serialized).expect("Failed to deserialize User");
        assert_eq!(user.id, deserialized.id);
        assert_eq!(user.email, deserialized.email);
        assert_eq!(user.username, deserialized.username);
    }

    #[test]
    fn test_login_request_serialization() {
        let req = LoginRequest {
            email: "user@example.com".to_string(),
            password: "password123".to_string(),
        };

        let json = serde_json::to_value(&req).expect("Failed to serialize");
        assert_eq!(json["email"], "user@example.com");
        assert_eq!(json["password"], "password123");
    }

    #[test]
    fn test_login_response_serialization() {
        let response = LoginResponse {
            token: "jwt-token-abc123".to_string(),
            refresh_token: Some("refresh-token-xyz789".to_string()),
            user: User {
                id: "user-1".to_string(),
                email: "test@example.com".to_string(),
                username: "testuser".to_string(),
            },
        };

        let serialized = serde_json::to_string(&response).expect("Failed to serialize");
        let deserialized: LoginResponse = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(response.token, deserialized.token);
        assert_eq!(response.user.id, deserialized.user.id);
        assert_eq!(response.user.email, deserialized.user.email);
    }

    #[test]
    fn test_database_serialization() {
        let db = Database {
            id: "db-1".to_string(),
            name: "My Notes".to_string(),
            description: "Personal notes database".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        };

        let serialized = serde_json::to_string(&db).expect("Failed to serialize Database");
        let deserialized: Database = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(db.id, deserialized.id);
        assert_eq!(db.name, deserialized.name);
        assert_eq!(db.description, deserialized.description);
    }

    #[test]
    fn test_note_serialization() {
        let note = Note {
            id: "note-1".to_string(),
            database_id: "db-1".to_string(),
            title: "Test Note".to_string(),
            content: "This is test content".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        };

        let serialized = serde_json::to_string(&note).expect("Failed to serialize Note");
        let deserialized: Note = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(note.id, deserialized.id);
        assert_eq!(note.database_id, deserialized.database_id);
        assert_eq!(note.title, deserialized.title);
        assert_eq!(note.content, deserialized.content);
    }

    #[test]
    fn test_nav_serialization() {
        let nav = Nav {
            id: "nav-1".to_string(),
            note_id: "note-1".to_string(),
            parent_id: Some("parent-nav".to_string()),
            content: "Navigation item".to_string(),
            position: 0,
        };

        let serialized = serde_json::to_string(&nav).expect("Failed to serialize Nav");
        let deserialized: Nav = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(nav.id, deserialized.id);
        assert_eq!(nav.note_id, deserialized.note_id);
        assert_eq!(nav.parent_id, deserialized.parent_id);
        assert_eq!(nav.content, deserialized.content);
        assert_eq!(nav.position, deserialized.position);
    }

    #[test]
    fn test_nav_without_parent() {
        let nav = Nav {
            id: "nav-1".to_string(),
            note_id: "note-1".to_string(),
            parent_id: None,
            content: "Root navigation".to_string(),
            position: 0,
        };

        let serialized = serde_json::to_string(&nav).expect("Failed to serialize");
        let deserialized: Nav = serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert!(deserialized.parent_id.is_none());
    }

    #[test]
    fn test_create_database_request() {
        let req = CreateDatabaseRequest {
            name: "New Database".to_string(),
            description: "A new database for notes".to_string(),
        };

        let json = serde_json::to_value(&req).expect("Failed to serialize");
        assert_eq!(json["name"], "New Database");
        assert_eq!(json["description"], "A new database for notes");
    }

    #[test]
    fn test_create_note_request() {
        let req = CreateNoteRequest {
            database_id: "db-1".to_string(),
            title: "New Note".to_string(),
        };

        let json = serde_json::to_value(&req).expect("Failed to serialize");
        assert_eq!(json["database_id"], "db-1");
        assert_eq!(json["title"], "New Note");
    }

    #[test]
    fn test_get_note_list_request() {
        let req = GetNoteListRequest {
            database_id: "db-1".to_string(),
            page: 1,
            page_size: 20,
        };

        let json = serde_json::to_value(&req).expect("Failed to serialize");
        assert_eq!(json["database_id"], "db-1");
        assert_eq!(json["page"], 1);
        assert_eq!(json["page_size"], 20);
    }

    #[test]
    fn test_signup_request_serialization() {
        let req = SignupRequest {
            email: "user@example.com".to_string(),
            username: "newuser".to_string(),
            password: "securepassword123".to_string(),
        };

        let json = serde_json::to_value(&req).expect("Failed to serialize");
        assert_eq!(json["email"], "user@example.com");
        assert_eq!(json["username"], "newuser");
        assert_eq!(json["password"], "securepassword123");
    }

    #[test]
    fn test_signup_response_deserialization() {
        let json_data = json!({
            "message": "User created successfully",
            "user": {
                "id": "user-123",
                "email": "newuser@example.com",
                "username": "newuser"
            }
        });

        let response: SignupResponse = serde_json::from_value(json_data).expect("Failed to deserialize");
        assert_eq!(response.message, "User created successfully");
        assert!(response.user.is_some());
        assert_eq!(response.user.unwrap().email, "newuser@example.com");
    }

    #[test]
    fn test_signup_response_without_user() {
        let json_data = json!({
            "message": "User created successfully",
            "user": null
        });

        let response: SignupResponse = serde_json::from_value(json_data).expect("Failed to deserialize");
        assert_eq!(response.message, "User created successfully");
        assert!(response.user.is_none());
    }

    #[test]
    fn test_refresh_token_request_serialization() {
        let req = RefreshTokenRequest {
            refresh_token: "refresh-token-abc123".to_string(),
        };

        let json = serde_json::to_value(&req).expect("Failed to serialize");
        assert_eq!(json["refresh_token"], "refresh-token-abc123");
    }

    #[test]
    fn test_refresh_token_response_deserialization() {
        let json_data = json!({
            "token": "new-jwt-token",
            "refresh_token": "new-refresh-token"
        });

        let response: RefreshTokenResponse = serde_json::from_value(json_data).expect("Failed to deserialize");
        assert_eq!(response.token, "new-jwt-token");
        assert_eq!(response.refresh_token, Some("new-refresh-token".to_string()));
    }

    #[test]
    fn test_refresh_token_response_without_refresh_token() {
        let json_data = json!({
            "token": "new-jwt-token",
            "refresh_token": null
        });

        let response: RefreshTokenResponse = serde_json::from_value(json_data).expect("Failed to deserialize");
        assert_eq!(response.token, "new-jwt-token");
        assert!(response.refresh_token.is_none());
    }

    #[test]
    fn test_login_response_with_refresh_token() {
        let json_data = json!({
            "token": "jwt-token",
            "refresh_token": "refresh-token",
            "user": {
                "id": "user-1",
                "email": "test@example.com",
                "username": "testuser"
            }
        });

        let response: LoginResponse = serde_json::from_value(json_data).expect("Failed to deserialize");
        assert_eq!(response.token, "jwt-token");
        assert_eq!(response.refresh_token, Some("refresh-token".to_string()));
        assert_eq!(response.user.username, "testuser");
    }

    #[test]
    fn test_login_response_without_refresh_token() {
        let json_data = json!({
            "token": "jwt-token",
            "refresh_token": null,
            "user": {
                "id": "user-1",
                "email": "test@example.com",
                "username": "testuser"
            }
        });

        let response: LoginResponse = serde_json::from_value(json_data).expect("Failed to deserialize");
        assert_eq!(response.token, "jwt-token");
        assert!(response.refresh_token.is_none());
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

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_app_state_new() {
        let state = AppState::new();
        assert!(!state.api_client.get_untracked().base_url.is_empty());
        assert!(state.current_user.get_untracked().is_none());
        assert!(state.databases.get_untracked().is_empty());
    }

    #[test]
    fn test_multiple_databases_deserialization() {
        let json_data = json!({
            "databases": [
                {
                    "id": "db-1",
                    "name": "Personal",
                    "description": "Personal notes",
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-02T00:00:00Z"
                },
                {
                    "id": "db-2",
                    "name": "Work",
                    "description": "Work notes",
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-02T00:00:00Z"
                }
            ]
        });

        let databases: Vec<Database> = serde_json::from_value(json_data["databases"].clone())
            .expect("Failed to deserialize databases");

        assert_eq!(databases.len(), 2);
        assert_eq!(databases[0].name, "Personal");
        assert_eq!(databases[1].name, "Work");
    }

    #[test]
    fn test_response_error_handling_format() {
        let error_response = json!({
            "error": "Invalid credentials",
            "code": 401
        });

        assert!(error_response["error"].is_string());
        assert_eq!(error_response["error"], "Invalid credentials");
    }
}
