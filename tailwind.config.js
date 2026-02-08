/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.{rs,html,js}"],
  theme: {
    extend: {
      colors: {
        background: "rgb(var(--background) / <alpha-value>)",
        foreground: "rgb(var(--foreground) / <alpha-value>)",

        // Surfaces
        muted: "rgb(var(--muted) / <alpha-value>)", // == surface
        surface: "rgb(var(--muted) / <alpha-value>)",
        "surface-hover": "rgb(var(--surface-hover) / <alpha-value>)",
        "surface-active": "rgb(var(--surface-active) / <alpha-value>)",

        // Text
        "muted-foreground": "rgb(var(--muted-foreground) / <alpha-value>)",
        "text-disabled": "rgb(var(--text-disabled) / <alpha-value>)",

        // Borders
        border: "rgb(var(--border) / <alpha-value>)",
        "border-strong": "rgb(var(--border-strong) / <alpha-value>)",

        // Accent
        accent: "rgb(var(--accent) / <alpha-value>)",
        "accent-hover": "rgb(var(--accent-hover) / <alpha-value>)",
        "accent-active": "rgb(var(--accent-active) / <alpha-value>)",
        "accent-foreground": "rgb(var(--accent-foreground) / <alpha-value>)",
        "accent-soft": "var(--accent-soft)",
        ring: "var(--ring)",

        // Aliases used by components
        input: "rgb(var(--border) / <alpha-value>)",
        primary: "rgb(var(--accent) / <alpha-value>)",
        "primary-foreground": "rgb(var(--accent-foreground) / <alpha-value>)",
        card: "rgb(var(--muted) / <alpha-value>)",
        "card-foreground": "rgb(var(--foreground) / <alpha-value>)",
      },
      borderRadius: {
        sm: "var(--radius-sm)",
        md: "var(--radius-md)",
        lg: "var(--radius-lg)",
      },
    },
  },
  plugins: [],
};
