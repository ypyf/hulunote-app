# Leptos 0.8.x Development Guide

This project targets **Leptos 0.8.x** (see `Cargo.toml`).

## Table of Contents

1. Quickstart & Essential Imports
2. Signals Basics
3. Rendering Rules (`view!`)
4. Event Handlers
5. Router / Params (CSR)
6. WASM/CSR & Build Gotchas
7. Appendix: Minimal Router Example

---

## 1) Quickstart & Essential Imports

```rust
use wasm_bindgen::prelude::*;           // For #[wasm_bindgen(start)]
use leptos::prelude::*;                 // Core: signal, RwSignal, view!, etc.
use leptos::task::spawn_local;          // Async tasks in WASM
use leptos_router::components::*;        // <Router/>, <Routes/>, <Route/>
use leptos_router::path;                // path!(...) macro
use leptos_router::hooks::use_location; // Location hooks (requires <Router>)
```

---

## 2) Signals Basics

### ReadSignal / WriteSignal

```rust
let (count, set_count) = signal(0);

// Read
let _ = count.get();

// Write
set_count.set(5);
```

Notes:
- `signal()` returns `(ReadSignal<T>, WriteSignal<T>)`.

### RwSignal

Use `RwSignal` when you want a **single handle** that supports both `.get()` and `.set()`.

```rust
let count: RwSignal<i32> = RwSignal::new(0);
count.set(1);
assert_eq!(count.get(), 1);
```

---

## 3) Rendering Rules (`view!`)

### 3.1 Render signal *values*, not signal *handles*

In Leptos 0.8, **do not render a signal handle directly** inside `view!`.

Bad:

```rust
let name: RwSignal<String> = RwSignal::new("abc".to_string());

view! {
  <div>{name}</div> // ❌ error[E0277]: RwSignal<String>: IntoRender is not satisfied
}
```

Good (render the signal *value* via a closure):

```rust
let name: RwSignal<String> = RwSignal::new("abc".to_string());

view! {
  <div>{move || name.get()}</div> // ✅ reactive
}
```

Notes:
- Use `{move || signal.get()}` (or `{move || read_signal.get()}`) to make the render reactive.
- This avoids errors like:
  - `error[E0277]: the trait bound RwSignal<String>: IntoRender is not satisfied`
  - `error[E0599]: method 'class' ... trait bounds were not satisfied` (often a follow-on error)

### 3.2 Tracked vs untracked reads (rule of thumb)

- **Inside `view!`**: prefer tracked reads via a closure (`move || signal.get()`).
- **Inside event handlers / async tasks**: prefer **untracked** reads when you want “the current value now” without creating reactive dependencies.

#### Common router pitfall: `use_location().pathname.get()` outside tracking

`use_location().pathname` is a reactive memo.
If you call `.get()` in a non-reactive context (e.g. inside `spawn_local`, callbacks, or plain component body code that is not used in `view!`/`Effect`), Leptos may warn:

> you access an ArcMemo outside a reactive tracking context

Fix: read the location **untracked** in those contexts.

```rust
let location = use_location();
let pathname = move || location.pathname.get();              // tracked (for view!/Effect)
let pathname_untracked = move || location.pathname.get_untracked(); // untracked (for handlers/async)

spawn_local(async move {
    if pathname_untracked().starts_with("/db/") {
        // ...
    }
});
```

---

## 4) Event Handlers

### 4.0 Disposed reactive values (common panic when navigation/unmount happens)

You may see a browser console panic like:

> panicked at reactive_graph ... you tried to access a reactive value ... but it has already been disposed

This comes from Leptos' underlying reactive runtime (`reactive_graph`).
It means: **some signal/memo/StoredValue belonged to an Owner/component scope that has been dropped**,
but a callback ran later and tried to read it.

Why it happens in apps like Hulunote:
- Pressing **Enter** can trigger navigation or a state change that **unmounts** the current node/editor.
- The input then fires a late **`blur`** (or other DOM event).
- If the `on:blur` handler reads tracked reactive state like `some_signal.get()` / `StoredValue::get_value()`,
  that read can happen *after unmount* → the reactive value is already disposed → panic.

Rule of thumb:
- **Inside event handlers that can run during teardown** (blur, keydown that navigates, callbacks scheduled with
  `set_timeout`, async tasks that outlive the component), avoid reading reactive values that are owned by the
  component scope.

Safer patterns:
1) **Capture plain data** (Strings/IDs) *before* starting navigation/unmount, then move them into the handler.
2) Or **read from the DOM event target** (e.g. attributes on the input) instead of the reactive graph.
3) Or guard: if you can't recover required ids safely, **do nothing** (skip the request) rather than sending
   an invalid request.

Hulunote case study (Enter → navigation/unmount → blur):
- Fix: store `data-nav-id` / `data-note-id` on the `<input>` while it is mounted.
- In `on:blur`, read ids from `ev.target()` (DOM) and **early-return if empty**.
- This prevents both the Leptos panic and backend 400s caused by empty ids.


### Input handlers

```rust
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

Notes:
- Closure signature must match the event type you bind.
- Avoid overly generic closures in `view!`.

### Async tasks (WASM)

```rust
use leptos::task::spawn_local;

spawn_local(async move {
    // async code here
});
```

`spawn_local` is required for WASM; `std::thread::spawn` does not work.

---

## 5) Router / Params (CSR)

Key points:
- Router hooks like `use_location()` **must** be called under a `<Router>`.
- Prefer defining routes via `<Routes>` + `<Route>` and `path!(...)`.

### Route params are reactive

`use_params()` returns reactive state. **Do not** read `params.get()` once in the component body and stash it in a plain variable.

Correct patterns:
- Read inside `view!` via a closure:

```rust
{move || params.get().ok().and_then(|p| p.id).unwrap_or_default()}
```

- Or define a derived closure:

```rust
let id = move || params.get().ok().and_then(|p| p.id).unwrap_or_default();
view! { <p>{move || id()}</p> }
```

In **event handlers / async tasks**, prefer **untracked** reads:

```rust
let id_now = params.get_untracked().ok().and_then(|p| p.id).unwrap_or_default();
```

---

## 6) WASM/CSR & Build Gotchas

### 6.1 Ensure CSR is enabled

Client-side rendering requires enabling the `csr` feature on the `leptos` crate.

```toml
[lib]
path = "src/lib.rs"
crate-type = ["cdylib", "rlib"]  # cdylib is REQUIRED

[dependencies]
leptos = { version = "0.8.x", features = ["csr"] }
leptos_router = "0.8.x"          # router uses default features

[dependencies.web-sys]
version = "0.3"
features = [
  "Window",
  "Document",
  "Element",
  "HtmlElement",
  "HtmlInputElement",
  "Event",
  "EventTarget",
]
```

If you forget `csr`, you may see a runtime warning like:

> It seems like you're trying to use Leptos in client-side rendering mode, but the `csr` feature is not enabled...

### 6.2 web-sys types must be enabled

Always match the `web-sys` feature to the type you use:
- `HtmlInputElement` for `<input>`
- `HtmlElement` for generic elements
- Event types like `KeyboardEvent`, `MouseEvent` need their own features if used

### 6.3 Build troubleshooting checklist

If a build error seems inconsistent across environments, first verify you’re building the **same commit** and doing a **clean build**.

Suggested checks:

```bash
git rev-parse HEAD
cargo clean
trunk build
```

Common mistake:
- Rendering a signal handle directly (`{some_rw_signal}`) instead of rendering its value via a closure (`{move || some_rw_signal.get()}`).

---

## 7) Appendix: Minimal Router Example

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

    view! { <div>{move || pathname()}</div> }
}
```

---

## App Entry Point (WASM)

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
pub struct AppState { /* ... */ }

#[derive(Clone)]
pub struct AppContext(pub AppState);

provide_context(AppContext(AppState::new()));
let app_state = expect_context::<AppContext>();
```
