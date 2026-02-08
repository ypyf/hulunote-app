# hulunote-app

A modern web client for [hulunote](https://github.com/hulunote/hulunote), an open-source Roam Research-style note-taking service.

## Overview

This client is built with **Leptos** (Rust WASM framework) and connects to the [hulunote-rust](https://github.com/hulunote/hulunote-rust) backend API.

## Tech Stack

- **Framework**: Leptos 0.7.x (Rust WASM)
- **Build Tool**: Trunk
- **Styling**: Tailwind CSS
- **Routing**: leptos_router
- **Backend**: hulunote-rust API

## Features

- [ ] Authentication (login/register)
- [ ] Database management
- [ ] Note editing with outliner
- [ ] Bidirectional links
- [ ] Daily notes
- [ ] Full-text search
- [ ] Import/Export (Markdown, JSON)
- [ ] MCP Client integration
- [ ] Settings page

## Getting Started

### Prerequisites

```bash
# Install Rust
rustup install stable

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install Trunk
cargo install trunk

# Install Tailwind CSS CLI (Trunk will invoke this to compile Tailwind)
brew install tailwindcss
```

### Development

```bash
# Start dev server at http://localhost:8080 (with auto-rebuild)
trunk serve
```

### Tests

```bash
# Unit tests (host)
cargo test
```

See the full test instructions (including WASM/WebDriver setup) in:
- [docs/TEST_GUIDE.md](./docs/TEST_GUIDE.md)

### Production Build

```bash
trunk build --release
```

### Environment Configuration

The app reads configuration from `window.ENV` in the browser. To customize the API URL:

```html
<script>
  window.ENV = {
    API_URL: "http://your-backend-url:6689"
  };
</script>
```

Or set the default in `src/lib.rs` via the `get_api_url()` function.

For local development, copy `.env.example` to `.env` and configure as needed.

## Project Structure

```
hulunote-app/
├── src/
│   └── lib.rs         # Main app (components, API client, state)
├── index.html          # Entry HTML (Trunk + Tailwind pipeline)
├── trunk.toml         # Trunk build configuration
├── tailwind.config.js # Tailwind CSS configuration
├── public/
│   └── style.css      # Global styles with Tailwind directives
├── Cargo.toml         # Dependencies and WASM config
└── docs/              # Documentation
    ├── PRODUCT.md     # Product overview
    ├── API_REFERENCE.md
    └── LEPTOS_GUIDE.md # Leptos development guide
```

## Documentation

- [Product Overview](./docs/PRODUCT.md)
- [API Reference](./docs/API_REFERENCE.md)
- [Leptos Development Guide](./docs/LEPTOS_GUIDE.md)
- [Test Guide](./docs/TEST_GUIDE.md)

## Backend Connection

Default backend URL: `http://localhost:6689`

## Desktop Build

To build for desktop, you have several options:

### Option 1: Tauri (Recommended)

Tauri can wrap the WASM application for native desktop deployment.

```bash
# Install Tauri CLI
cargo install tauri-cli

# Build for desktop
cargo tauri build
```

### Option 2: Web Desktop Wrappers

For a more lightweight desktop experience, consider:
- [nativefier](https://github.com/nativefier/nativefier) - Wrap the web app as a desktop app
- [Electron](https://www.electronjs.org/) - Create a desktop wrapper

## License

MIT
