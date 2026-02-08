use leptos::prelude::*;
use leptos_ui::clx;

mod components {
    use super::*;
    clx! {Card, div, "bg-card text-card-foreground flex flex-col gap-4 rounded-xl border py-6 shadow-sm"}
    // TODO. Change data-slot=card-action by data-name="CardAction".
    clx! {CardHeader, div, "@container/card-header flex flex-col items-start gap-1.5 px-6 [.border-b]:pb-6 sm:grid sm:auto-rows-min sm:grid-rows-[auto_auto] has-data-[slot=card-action]:sm:grid-cols-[1fr_auto]"}
    clx! {CardTitle, h2, "leading-none font-semibold"}
    clx! {CardContent, div, "px-6"}
    clx! {CardDescription, p, "text-muted-foreground text-sm"}
    clx! {CardFooter, footer, "flex items-center px-6 [.border-t]:pt-6", "gap-2"}

    clx! {CardAction, div, "self-start sm:col-start-2 sm:row-span-2 sm:row-start-1 sm:justify-self-end"}
    clx! {CardList, ul, "flex flex-col gap-4"}
    clx! {CardItem, li, "flex items-center [&_svg:not([class*='size-'])]:size-4 [&_svg]:shrink-0"}
}

#[allow(unused_imports)]
pub use components::*;
