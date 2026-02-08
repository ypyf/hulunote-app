use leptos::prelude::*;
use leptos_ui::variants;

// TODO 💪 Loading state (demo_use_timeout_fn.rs and demo_button.rs)

variants! {
    Button {
        // shadcn-ish defaults, but mapped to our token names (primary/accent/surface/border)
        base: "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:ring-ring focus-visible:ring-[3px] hover:cursor-pointer touch-manipulation [-webkit-tap-highlight-color:transparent] select-none [-webkit-touch-callout:none]",
        variants: {
            variant: {
                Default: "bg-primary text-primary-foreground shadow-xs hover:bg-accent-hover",
                Destructive: "bg-destructive text-destructive-foreground shadow-xs hover:bg-destructive/90 focus-visible:ring-destructive/30",
                Outline: "border border-border bg-background shadow-xs hover:bg-surface-hover hover:text-foreground",
                Secondary: "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80",
                Ghost: "hover:bg-accent-soft hover:text-foreground",
                Accent: "bg-accent text-accent-foreground shadow-xs hover:bg-accent-hover",
                Link: "text-primary underline-offset-4 hover:underline",
                Warning: "bg-warning text-warning-foreground shadow-xs hover:bg-warning/90",
                Success: "bg-success text-success-foreground shadow-xs hover:bg-success/90",
                Bordered: "bg-transparent border border-border text-muted-foreground hover:bg-surface-hover hover:text-foreground",
            },
            size: {
                Default: "h-9 px-4 py-2 has-[>svg]:px-3",
                Sm: "h-8 rounded-md gap-1.5 px-3 has-[>svg]:px-2.5",
                Lg: "h-10 rounded-md px-6 has-[>svg]:px-4",
                Icon: "size-9",
                Mobile: "px-6 py-3 rounded-[24px]",
                Badge: "px-2.5 py-0.5 text-xs",
            }
        },
        component: {
            element: button,
            support_href: true,
            support_aria_current: true
        }
    }
}
