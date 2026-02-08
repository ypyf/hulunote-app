# Leptos 0.7.x Development Guide

## Essential Imports

```rust
use wasm_bindgen::prelude::*;        // For #[wasm_bindgen(start)]
use leptos::prelude::*;              // Core: signal, RwSignal, view!, etc.
use leptos::task::spawn_local;       // Async tasks in WASM
use leptos_router::hooks::use_location; // Routing
```

## Signals: Creation and Usage

**Wrong**: `let count = signal(0);`
**Correct**: `let (count, set_count) = signal(0);`

- `signal()` returns a tuple: `(ReadSignal<T>, WriteSignal<T>)`
- Use `.get()` to read: `count.get()`
- Use `.set(value)` to write: `set_count.set(5)`
- For closures, use the setter directly in event handlers

## RwSignal for Unified Get/Set

```rust
let count: RwSignal<i32> = RwSignal::new(0);
count.set(1);
count.get(); // Returns 0, but .read() gives actual value
```

Use when you need to pass a single signal value around.

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

## Cargo.toml for WASM

```toml
[lib]
path = "src/lib.rs"
crate-type = ["cdylib", "rlib"]  # cdylib is REQUIRED

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
    leptos::mount_to_body(App);
}
```

Note: `mount_to_body` not `leptos::mount_to_body` when imported via `use leptos::prelude::*`.

## Context Pattern

```rust
#[derive(Clone)]
pub struct AppState { ... }

#[derive(Clone)]
pub struct AppContext(pub AppState);

provide_context(AppContext(AppState::new()));
let app_state = expect_context::<AppContext>();
```

## Leptos Router

```rust
use leptos_router::hooks::use_location;

let location = use_location();
let pathname = move || location.pathname.get();
```

Routes are checked via string matching, not components.
