# Leptos 0.8.x Development Guide

This project currently targets Leptos 0.8.x (see `Cargo.toml`).


## Essential Imports

```rust
use wasm_bindgen::prelude::*;        // For #[wasm_bindgen(start)]
use leptos::prelude::*;              // Core: signal, RwSignal, view!, etc.
use leptos::task::spawn_local;       // Async tasks in WASM
use leptos_router::components::*;       // <Router/>, <Routes/>, <Route/>
use leptos_router::path;                // path!(...) macro
use leptos_router::hooks::use_location; // Location hooks (requires <Router>)
```

## Signals: Creation and Usage

## Rendering Signals in `view!` (WASM/CSR)

In Leptos 0.8, **do not render a signal handle directly** inside `view!`.

Bad (may compile in some host test builds, but fails in `wasm32-unknown-unknown` / Trunk CI):

```rust
let name: RwSignal<String> = RwSignal::new("abc".to_string());

view! {
  <div>{name}</div> // ❌ error[E0277]: RwSignal<String>: IntoRender is not satisfied
}
```

Good (render the signal *value* reactively via a closure):

```rust
let name: RwSignal<String> = RwSignal::new("abc".to_string());

view! {
  <div>{move || name.get()}</div> // ✅
}
```

Notes:
- Use `{move || signal.get()}` (or `{move || signal()}` for `ReadSignal`) to make it reactive.
- This avoids CI-only failures like:
  - `error[E0277]: the trait bound RwSignal<String>: IntoRender is not satisfied`
  - `error[E0599]: method 'class' ... trait bounds were not satisfied` (often a follow-on error)

If you see this in CI but not locally, ensure you're testing the same target:

```bash
trunk build  # builds wasm32-unknown-unknown
```

**Correct (Leptos 0.8)**:

```rust
let (count, set_count) = signal(0);

// Read
let _ = count.get();

// Write
set_count.set(5);
```

Notes:
- `signal()` returns `(ReadSignal<T>, WriteSignal<T>)`.
- In `view!` closures, prefer `move || count.get()` for reactive reads.

## RwSignal for Unified Get/Set

```rust
let count: RwSignal<i32> = RwSignal::new(0);
count.set(1);
assert_eq!(count.get(), 1);
```

Use `RwSignal` when you want a single handle that supports both `.get()` and `.set()`.

## Async Tasks

```rust
use leptos::task::spawn_local;

spawn_local(async move {
    // async code here
});
```

`spawn_local` is required for WASM - `std::thread::spawn` doesn't work.

## Event Handlers in view!

```rust
// Correct pattern for input handlers
let handle_input = move |e: web_sys::Event| {
    if let Some(target) = e.target() {
        if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
            signal.set(input.value());
        }
    }
};

view! {
    <input on:input=handle_input />
}
```

Closure must match expected signature. Avoid generic closures in view!.

## Cargo.toml for WASM / CSR

Client-side rendering (CSR) in Leptos requires enabling the `csr` feature on the `leptos` crate.

```toml
[lib]
path = "src/lib.rs"
crate-type = ["cdylib", "rlib"]  # cdylib is REQUIRED

[dependencies]
# Enable client-side rendering
leptos = { version = "0.8.x", features = ["csr"] }

# Router does not have a `csr` feature; use the default crate features
leptos_router = "0.8.x"

[dependencies.web-sys]
version = "0.3"
features = [
    "Window",
    "Document",
    "Element",
    "HtmlElement",
    "HtmlInputElement",  # Enable specific types you use
    "Event",
    "EventTarget",
]
```

If you forget to enable `csr`, you will see a runtime warning like:

> It seems like you're trying to use Leptos in client-side rendering mode, but the `csr` feature is not enabled...

## web-sys Types

Always match the web-sys feature to the type you need:
- `HtmlInputElement` for `<input>` elements
- `HtmlElement` for generic HTML elements
- Event types like `KeyboardEvent`, `MouseEvent` need their own features

## App Entry Point

```rust
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
```

(Requires `use leptos::prelude::*;` to bring `mount_to_body` into scope.)

## Context Pattern

```rust
#[derive(Clone)]
pub struct AppState { ... }

#[derive(Clone)]
pub struct AppContext(pub AppState);

provide_context(AppContext(AppState::new()));
let app_state = expect_context::<AppContext>();
```

## Leptos Router (CSR)

Key points:
- Router hooks like `use_location()` **must** be called under a `<Router>`.
- Prefer defining routes via `<Routes>` + `<Route>` and `path!(...)`.
- **Route params are reactive** (`use_params()` returns a memo/signal). Do **not** read `params.get()` in the component body and stash the value in a plain variable.
  - This triggers warnings like: "access ... outside a reactive tracking context"
  - And it may stop your UI from updating when the route changes.
  - Correct patterns:
    - Read inside `view!` via a closure: `{move || params.get().ok().and_then(|p| p.id).unwrap_or_default()}`
    - Or define a derived closure first:
      ```rust
      let id = move || params.get().ok().and_then(|p| p.id).unwrap_or_default();
      view! { <p>{move || id()}</p> }
      ```
  - In **event handlers / async tasks**, prefer **untracked** reads (you want the current value, not a reactive dependency):
      ```rust
      let id_now = params.get_untracked().ok().and_then(|p| p.id).unwrap_or_default();
      ```
  - If you intentionally want a one-time read, use `get_untracked()` / `with_untracked()` and accept it won't react to route changes.

Minimal example:

```rust
use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::use_location;
use leptos_router::path;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Routes fallback=|| "Not found">
                <Route path=path!("login") view=|| view! { "Login" } />
                <Route path=path!("") view=|| view! { "Home" } />
            </Routes>
        </Router>
    }
}

#[component]
pub fn SomeChild() -> impl IntoView {
    let location = use_location();
    let pathname = move || location.pathname.get();

    view! { <div>{pathname}</div> }
}
```
