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
```

### Development

```bash
# Start dev server at http://localhost:8080
trunk serve
```

### Production Build

```bash
trunk build
```

## Project Structure

```
hulunote-app/
├── src/
│   └── lib.rs         # Main app (components, API client, state)
├── index.html          # Entry HTML with Tailwind
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

## Backend Connection

Default backend URL: `http://localhost:6689`

## License

MIT
