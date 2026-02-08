# Hulunote Product Documentation

## 1. Product Overview

### 1.1 Introduction
Hulunote is an open-source outliner note-taking application inspired by Roam Research, designed for Networked Thought. Users can organize information through hierarchical bullet note structures and connect different notes using bidirectional links.

### 1.2 Core Value
- **Structured Thinking**: Organize information with infinitely nested hierarchical structures
- **Knowledge Network**: Bidirectional links connect ideas across notes
- **Quick Capture**: Minimalist input experience with daily notes support
- **Multiple Workspaces**: Independent databases for different projects

### 1.3 Target Users
- Knowledge workers and researchers
- Professionals managing complex information
- Users who practice networked thinking
- Developers and technical writers

---

## 2. Features

### 2.1 Core Features

#### 2.1.1 Outliner Structure
- **Infinite Nesting**: Unlimited hierarchical bullet note structure
- **Drag & Drop**: Rearrange nodes by dragging to adjust position and hierarchy
- **Collapse/Expand**: Click collapse indicator to toggle node visibility
- **Quick Indent**: Tab to increase indent, Shift+Tab to decrease

#### 2.1.2 Bidirectional Links
- **Wiki-style Links**: Use `[[Page Name]]` to create links
- **Backlinks**: Show all pages referencing the current page at the bottom
- **Link Preview**: Hover over links to see page summary
- **Unreferenced Pages**: Display orphaned pages not linked from anywhere

#### 2.1.3 Daily Notes
- **Auto-create**: Click today's date to automatically create or open today's note
- **Date Formatting**: Support various date formats (YYYYMMDD, YYYY-MM-DD, etc.)
- **Journal Templates**: Configurable daily note templates
- **Quick Indexing**: Quickly navigate historical notes by date

#### 2.1.4 Multiple Databases
- **Independent Workspaces**: Each database is completely separate
- **Database Switching**: Quick sidebar switching between databases
- **Database Management**: Create, delete, rename databases
- **Database Sharing**: Support export and import

#### 2.1.5 MCP Client
- **Model Context Protocol**: Experimental MCP integration
- **AI Chat Interface**: Built-in AI conversation
- **Tool Configuration**: Configurable MCP tools
- **Context Sharing**: AI can access current note context

### 2.2 Auxiliary Features

#### 2.2.1 Search
- **Full-text Search**: Quickly search all note content
- **Title Search**: Search only page titles
- **Advanced Search**: Support regex and filters
- **Search Shortcuts**: Global search hotkey

#### 2.2.2 Keyboard Shortcuts
- **Navigation Shortcuts**: Quick movement between nodes
- **Edit Shortcuts**: Common edit operations
- **Formatting Shortcuts**: Text formatting
- **Custom Shortcuts**: User-defined shortcuts

#### 2.2.3 Import/Export
- **Import Formats**: Markdown, HTML, OPML
- **Export Formats**: Markdown, PDF, HTML
- **Bulk Export**: Export entire database
- **Incremental Import**: Merge duplicate pages on import

---

## 3. User Interface

### 3.1 Overall Layout

```
+-------------------------------------------------------------+
|  +---------+---------------------------------------------+  |
|  | Sidebar |            Main Content Area                |  |
|  |         |                                             |  |
|  | - Nav   |  +-------------------------------------+   |  |
|  | - Files |  |         Outline View                |   |  |
|  | - Search|  |                                     |   |  |
|  | - Settings| |  · Root Node                     |   |  |
|  |         |  |    · Child Node 1                 |   |  |
|  |         |  |      · Grandchild Node            |   |  |
|  |         |  |    · Child Node 2                 |   |  |
|  |         |  |                                     |   |  |
|  |         |  +-------------------------------------+   |  |
|  |         |                                           |  |
|  +---------+-------------------------------------------+  |
+-------------------------------------------------------------+
```

### 3.2 Sidebar
- **Database List**: All databases for current user
- **Page Tree**: Page structure of current database
- **Global Search**: Search box at top
- **Settings Entry**: User settings and preferences

### 3.3 Main Content Area
- **Outline Editor**: Primary note editing area
- **Reference Panel**: Bidirectional link references
- **Toolbar**: Common action buttons

### 3.4 Design System

#### 3.4.1 Color Themes
- **Light Mode**: White background, black text (default)
- **Dark Mode**: Dark gray background, white text
- **High Contrast**: For visually impaired users

#### 3.4.2 Fonts
- **Body Font**: System default sans-serif
- **Code Font**: Monospace for code blocks
- **Heading Font**: For page titles

#### 3.4.3 Spacing
- **Compact Mode**: Minimum spacing
- **Comfortable Mode**: Default spacing
- **Loose Mode**: Maximum spacing

---

## 4. Data Model

### 4.1 Core Entities

#### 4.1.1 Account
| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Unique identifier |
| email | String | Email address |
| username | String | Username |
| password_hash | String | Hashed password |
| expires_at | Timestamp | Account expiration |
| created_at | Timestamp | Creation time |

#### 4.1.2 Database
| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Unique identifier |
| account_id | UUID | Owner account |
| name | String | Database name |
| description | String | Database description |
| created_at | Timestamp | Creation time |
| updated_at | Timestamp | Update time |

#### 4.1.3 Note
| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Unique identifier |
| database_id | UUID | Parent database |
| title | String | Note title |
| content | String | Note content (rich text) |
| created_at | Timestamp | Creation time |
| updated_at | Timestamp | Update time |

#### 4.1.4 Nav (Outline Node)
| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Unique identifier |
| note_id | UUID | Parent note |
| parent_id | UUID | Parent node |
| content | String | Node content |
| position | Int | Sort order |
| created_at | Timestamp | Creation time |
| updated_at | Timestamp | Update time |

#### 4.1.5 Block
| Field | Type | Description |
|-------|------|-------------|
| id | UUID | Unique identifier |
| note_id | UUID | Parent note |
| type | String | Block type (text, code, image, etc.) |
| content | String | Block content |
| properties | JSON | Block properties |
| position | Int | Sort order |

### 4.2 Relationship Diagram

```
Account 1 ───> N Database
Database 1 ───> N Note
Note 1 ───> N Nav (self-referencing)
Note 1 ───> N Block
```

### 4.3 Bidirectional Link Model

```
Page A ──[[Page B]]────> Page B
                                ^
                                |
                                └────[[Page A]]── Page C
```

---

## 5. API Endpoints

### 5.1 Authentication

#### 5.1.1 Login
```
POST /login/web-login
Content-Type: application/json

Request:
{
  "email": "user@example.com",
  "password": "password123"
}

Response (success):
{
  "token": "jwt_token",
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "username": "username"
  }
}
```

#### 5.1.2 Register
```
POST /login/web-signup
Content-Type: application/json

Request:
{
  "email": "user@example.com",
  "password": "password123",
  "username": "username",
  "registration_code": "FA8E-AF6E-4578-9347"
}

Response (success):
{
  "token": "jwt_token",
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "username": "username"
  }
}
```

### 5.2 Database Endpoints

#### 5.2.1 Create Database
```
POST /hulunote/new-database
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "name": "My Notebook",
  "description": "My personal notes"
}

Response:
{
  "database": {
    "id": "uuid",
    "name": "My Notebook",
    "description": "My personal notes"
  }
}
```

#### 5.2.2 Get Database List
```
POST /hulunote/get-database-list
Authorization: Bearer <token>
Content-Type: application/json

Response:
{
  "databases": [
    {
      "id": "uuid",
      "name": "My Notebook",
      "description": "My personal notes",
      "created_at": "timestamp"
    }
  ]
}
```

### 5.3 Note Endpoints

#### 5.3.1 Create Note
```
POST /hulunote/new-note
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "uuid",
  "title": "My Note"
}

Response:
{
  "note": {
    "id": "uuid",
    "database_id": "uuid",
    "title": "My Note",
    "content": ""
  }
}
```

#### 5.3.2 Update Note
```
POST /hulunote/update-hulunote-note
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "note_id": "uuid",
  "title": "Updated Title",
  "content": "Updated content"
}

Response:
{
  "success": true
}
```

#### 5.3.3 Get Note List (Paginated)
```
POST /hulunote/get-note-list
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "uuid",
  "page": 1,
  "page_size": 20
}

Response:
{
  "notes": [
    {
      "id": "uuid",
      "title": "Note Title",
      "content": "...",
      "created_at": "timestamp",
      "updated_at": "timestamp"
    }
  ],
  "total": 100,
  "page": 1,
  "page_size": 20
}
```

### 5.4 Outline Node Endpoints

#### 5.4.1 Create/Update Node
```
POST /hulunote/create-or-update-nav
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "note_id": "uuid",
  "nav_id": "uuid (optional, omit for create)",
  "content": "Node content",
  "parent_id": "parent_uuid_or_null"
}

Response:
{
  "nav": {
    "id": "uuid",
    "note_id": "uuid",
    "parent_id": "uuid_or_null",
    "content": "Node content",
    "position": 0
  }
}
```

#### 5.4.2 Get All Nodes
```
POST /hulunote/get-all-navs
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "uuid"
}

Response:
{
  "navs": [
    {
      "id": "uuid",
      "note_id": "uuid",
      "parent_id": "uuid_or_null",
      "content": "Node content",
      "position": 0
    }
  ]
}
```

---

## 6. User Flows

### 6.1 Registration Flow

```
1. User visits registration page
   |
   v
2. Enter email, password, username
   |
   v
3. Enter registration code
   |
   v
4. System validates registration code
   |
   +-- success --> Create account, send verification email
   |
   +-- failure --> Return error message
   |
   v
5. Email verification (optional)
   |
   v
6. Complete registration, redirect to homepage
```

### 6.2 Login Flow

```
1. User visits login page
   |
   v
2. Enter email and password
   |
   v
3. System validates credentials
   |
   +-- success --> Generate JWT Token
   |
   +-- failure --> Return error message
   |
   v
4. Store token (localStorage/Cookie)
   |
   v
5. Redirect to dashboard
```

### 6.3 Create Note Flow

```
1. Select database in sidebar
   |
   v
2. Click "New Note" button
   |
   v
3. Enter note title
   |
   v
4. Edit content in outline
   |
   v
5. Use [[link]] to create bidirectional links
   |
   v
6. Save (auto-save)
```

### 6.4 Organize Information Flow

```
1. Create root node (main topic)
   |
   v
2. Add child nodes (use Tab to indent)
   |
   v
3. Create links between child nodes
   |
   v
4. Use daily notes to capture thoughts
   |
   v
5. Discover connections through links
```

---

## 7. Technical Architecture

### 7.1 Frontend Architecture

#### 7.1.1 Tech Stack
- **ClojureScript**: Functional programming language
- **Reagent**: React wrapper for ClojureScript
- **Datascript**: In-memory database
- **Shadow-cljs**: Build tool
- **Electron**: Desktop application framework

#### 7.1.2 Frontend Structure
```
src/
├── cljs/
│   ├── main/
│   │   ├── core/          # Core functionality
│   │   ├── ui/            # UI components
│   │   ├── db/            # State management
│   │   ├── router/        # Routing
│   │   └── util/          # Utility functions
│   └── ...
├── resources/
│   └── public/            # Static assets
└── electron/              # Electron main process
```

#### 7.1.3 Data Flow
```
User Action -> Event Dispatcher -> State Update -> Re-render UI
```

### 7.2 Backend Architecture

#### 7.2.1 Tech Stack
- **Rust**: Programming language
- **Axum**: Web framework
- **SQLx**: SQL toolkit
- **PostgreSQL**: Database
- **JWT**: Authentication

#### 7.2.2 Backend Structure
```
src/
├── main.rs                # Entry point
├── config.rs              # Configuration
├── db.rs                  # Database
├── models.rs              # Data models
├── handlers/              # Handlers
│   ├── auth.rs            # Authentication
│   ├── database.rs        # Database operations
│   ├── note.rs            # Note operations
│   └── nav.rs             # Outline node operations
└── middleware.rs          # Middleware
```

### 7.3 Deployment Architecture

```
+---------------------------------------------------------------+
|                         Nginx                                 |
|                      (Reverse Proxy)                         |
+----------------------------+----------------------------------+
                           |
           +---------------+---------------+---------------+
           |               |               |               |
           v               v               v               v
     +---------+      +-----------+      +-----------+
     |  Rust   |      |PostgreSQL |      |  Static   |
     | Backend |      | Database  |      |   Files   |
     +---------+      +-----------+      +-----------+
```

---

## 8. Development Guide

### 8.1 Environment Setup

#### 8.1.1 Frontend Development
```bash
# Install dependencies
npm install

# Start dev server
npx shadow-cljs watch hulunote

# Build production
npx shadow-cljs release hulunote
```

#### 8.1.2 Backend Development
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Create database
createdb -U postgres hulunote_open
psql -U postgres -d hulunote_open -f init.sql

# Run backend
cargo run
```

### 8.2 Test Account
- **Email**: chanshunli@gmail.com
- **Password**: 123456

### 8.3 Configuration

#### 8.3.1 Backend API URL
Configure in `shadow-cljs.edn`:
```clojure
:hulunote.http/API_BASE_URL "http://localhost:6689"
```

#### 8.3.2 Environment Variables
```env
DATABASE_URL=postgres://postgres:password@localhost:5432/hulunote_open
JWT_SECRET=your-secret-key
PORT=6689
```

---

## 9. Appendix

### 9.1 Glossary

| Term | Description |
|------|-------------|
| Outliner | Outline editor |
| Block | Minimum unit of a note |
| Nav | Outline node |
| Bidirectional Link | Two-way link between pages |
| Backlink | Link pointing back to current page |
| Database | Workspace |
| Registration Code | Code for account registration |
| JWT | JSON Web Token |

### 9.2 References
- [Original Frontend](https://github.com/hulunote/hulunote)
- [Backend](https://github.com/hulunote/hulunote-rust)
- [Roam Research](https://roamresearch.com/)
- [Model Context Protocol](https://modelcontextprotocol.io/)

### 9.3 Changelog
- v0.1.0: First release (2026-02-07)
