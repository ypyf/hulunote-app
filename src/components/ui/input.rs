#![allow(dead_code)]

use leptos::html;
use leptos::prelude::*;
use tw_merge::tw_merge;
use wasm_bindgen::JsCast;

#[allow(dead_code)]
#[component]
pub fn Input(
    // Styling
    #[prop(into, optional)] class: String,

    // Common HTML attributes
    #[prop(into, default = "text")] r#type: &'static str,
    #[prop(into, optional)] placeholder: Option<String>,
    #[prop(into, optional)] name: Option<String>,
    #[prop(into, optional)] id: Option<String>,
    #[prop(optional)] disabled: bool,
    #[prop(optional)] readonly: bool,
    #[prop(optional)] required: bool,
    #[prop(optional)] autofocus: bool,

    // Two-way binding
    //
    // NOTE: We intentionally avoid `bind:value=...` here because Leptos binding
    // APIs/macros have changed across versions, and Trunk builds for wasm32 in CI.
    // This manual wiring is stable.
    #[prop(into, optional)] bind_value: Option<RwSignal<String>>,

    // Ref for direct DOM access
    #[prop(optional)] node_ref: NodeRef<html::Input>,
) -> impl IntoView {
    let merged_class = tw_merge!(
        "file:text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground dark:bg-input/30 border-input flex h-9 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none file:inline-flex file:h-7 file:border-0 file:bg-transparent file:text-sm file:font-medium disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm",
        "focus-visible:border-ring focus-visible:ring-ring/50",
        "focus-visible:ring-2",
        "aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive",
        "read-only:bg-muted",
        class
    );

    match bind_value {
        Some(signal) => {
            let on_input = move |ev: web_sys::Event| {
                if let Some(target) = ev.target() {
                    if let Some(input) = target.dyn_ref::<web_sys::HtmlInputElement>() {
                        signal.set(input.value());
                    }
                }
            };

            view! {
                <input
                    data-name="Input"
                    type=r#type
                    class=merged_class
                    placeholder=placeholder
                    name=name
                    id=id
                    disabled=disabled
                    readonly=readonly
                    required=required
                    autofocus=autofocus
                    value=move || signal.get()
                    on:input=on_input
                    node_ref=node_ref
                />
            }
            .into_any()
        }
        None => view! {
            <input
                data-name="Input"
                type=r#type
                class=merged_class
                placeholder=placeholder
                name=name
                id=id
                disabled=disabled
                readonly=readonly
                required=required
                autofocus=autofocus
                node_ref=node_ref
            />
        }
        .into_any(),
    }
}
