# Hulunote API Quick Reference

## Base Information

- **Base URL**: `http://localhost:6689`
- **Authentication**: JWT Bearer Token
- **Content-Type**: `application/json`

## Authentication Endpoints

### Login
```http
POST /login/web-login
Content-Type: application/json

Request:
{
  "email": "user@example.com",
  "password": "password123"
}

Response (success) (hulunote-rust):
{
  "token": "<jwt>",
  "hulunote": { /* account info object (backend-defined fields) */ },
  "region": null
}
```

### Register
```http
POST /login/web-signup
Content-Type: application/json

Request:
{
  "email": "user@example.com",
  "password": "password123",
  "username": "username",
  "registration_code": "FA8E-AF6E-4578-9347"
}

Response (success) (hulunote-rust):
{
  "token": "<jwt>",
  "hulunote": { /* account info object (backend-defined fields) */ },
  "database": "<db_name>",
  "region": null
}
```

## Database Endpoints

> Note: hulunote-rust responses/requests are **kebab-case** / legacy-compatible.
> In the wild you may see both the “new” (`databases`) and “legacy” (`database-list`) response shapes.
>
> Auth header may be either:
> - `Authorization: Bearer <jwt>` (hulunote-rust documented)
> - `X-FUNCTOR-API-TOKEN: <jwt>` (legacy client)

### Get Database List
```http
POST /hulunote/get-database-list
Authorization: Bearer <token>
Content-Type: application/json

Request:
{}

Response (hulunote-rust):
{
  "database-list": [
    {
      "hulunote-databases/id": "0a1dd8e1-e255-4b35-937e-bac27dea1274",
      "hulunote-databases/name": "My Notebook",
      "hulunote-databases/description": "My personal notes",
      "hulunote-databases/account-id": 3,
      "hulunote-databases/is-default": true,
      "hulunote-databases/is-delete": false,
      "hulunote-databases/is-public": false,
      "hulunote-databases/created-at": "2026-02-08T15:59:24.130460+00:00",
      "hulunote-databases/updated-at": "2026-02-08T15:59:24.130460+00:00"
    }
  ],
  "settings": {}
}
```

### Create Database
```http
POST /hulunote/new-database
Authorization: Bearer <token>
Content-Type: application/json

Request (hulunote-rust handler expects these keys):
{
  "database-name": "New Database",
  "description": "Description here"
}

(Backend struct uses `database_name` internally, but it is deserialized from kebab-case.)

Response:
{
  "database": {
    "hulunote-databases/id": "...",
    "hulunote-databases/name": "New Database",
    "hulunote-databases/description": "Description here",
    "hulunote-databases/created-at": "...",
    "hulunote-databases/updated-at": "..."
  },
  "success": true
}
```

### Update Database
```http
POST /hulunote/update-database
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "<uuid>",
  "db_name": "Updated Name"
}

Notes:
- The backend accepts `database_id` **or** `id`.
- Currently supports updating: `db_name` (name), `is_public`, `is_default`, `is_delete`.
- **Does not update description** (as of current hulunote-rust handler).

Response:
{
  "success": true
}
```

### Delete Database (Soft Delete)
```http
POST /hulunote/delete-database
Authorization: Bearer <token>
Content-Type: application/json

Request (either id or name):
{
  "database-id": "<uuid>"
}

OR
{
  "database-name": "My Notebook"
}

Response:
{
  "success": true,
  "message": "Database deleted successfully"
}
```

## Note Endpoints

### Create Note
```http
POST /hulunote/new-note
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "My Note"
}

Response:
{
  "note": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "database_id": "550e8400-e29b-41d4-a716-446655440000",
    "title": "My Note",
    "content": "",
    "created_at": "2026-02-08T10:00:00Z",
    "updated_at": "2026-02-08T10:00:00Z"
  }
}
```

### Get Note List (Paginated)
```http
POST /hulunote/get-note-list
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "550e8400-e29b-41d4-a716-446655440000",
  "page": 1,
  "page_size": 20
}

Response:
{
  "notes": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "Note Title",
      "content": "Content here",
      "created_at": "2026-02-08T10:00:00Z",
      "updated_at": "2026-02-08T10:00:00Z"
    }
  ],
  "total": 100,
  "page": 1,
  "page_size": 20
}
```

### Get All Notes
```http
POST /hulunote/get-all-note-list
Authorization: Bearer <token>
Content-Type: application/json

Request (backend expects `database_id`):
{
  "database_id": "550e8400-e29b-41d4-a716-446655440000"
}

Response:
{
  "notes": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "title": "Note Title",
      "content": "Content here",
      "created_at": "2026-02-08T10:00:00Z",
      "updated_at": "2026-02-08T10:00:00Z"
    }
  ]
}
```

### Update Note
```http
POST /hulunote/update-hulunote-note
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "note_id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Updated Title",
  "content": "Updated content"
}

Response:
{
  "success": true
}
```

## Outline Node Endpoints

### Create/Update Node
```http
POST /hulunote/create-or-update-nav
Authorization: Bearer <token>
Content-Type: application/json

Request (create):
{
  "note_id": "550e8400-e29b-41d4-a716-446655440000",
  "content": "Node content"
}

Request (update):
{
  "note_id": "550e8400-e29b-41d4-a716-446655440000",
  "nav_id": "660e8400-e29b-41d4-a716-446655440000",
  "content": "Updated content",
  "parent_id": "660e8400-e29b-41d4-a716-446655440001"
}

Response:
{
  "nav": {
    "id": "660e8400-e29b-41d4-a716-446655440000",
    "note_id": "550e8400-e29b-41d4-a716-446655440000",
    "parent_id": null,
    "content": "Node content",
    "position": 0
  }
}
```

### Get All Nodes for a Note
```http
POST /hulunote/get-note-navs
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "note_id": "550e8400-e29b-41d4-a716-446655440000"
}

Response:
{
  "navs": [
    {
      "id": "660e8400-e29b-41d4-a716-446655440000",
      "note_id": "550e8400-e29b-41d4-a716-446655440000",
      "parent_id": null,
      "content": "Root node",
      "position": 0
    }
  ]
}
```

### Get All Nodes (Paginated)
```http
POST /hulunote/get-all-nav-by-page
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "550e8400-e29b-41d4-a716-446655440000",
  "page": 1,
  "page_size": 100
}

Response:
{
  "navs": [
    {
      "id": "660e8400-e29b-41d4-a716-446655440000",
      "note_id": "550e8400-e29b-41d4-a716-446655440000",
      "parent_id": null,
      "content": "Root node",
      "position": 0
    }
  ],
  "total": 500,
  "page": 1,
  "page_size": 100
}
```

### Get All Nodes
```http
POST /hulunote/get-all-navs
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "550e8400-e29b-41d4-a716-446655440000"
}

Response:
{
  "navs": [
    {
      "id": "660e8400-e29b-41d4-a716-446655440000",
      "note_id": "550e8400-e29b-41d4-a716-446655440000",
      "parent_id": null,
      "content": "Root node",
      "position": 0
    }
  ]
}
```

## Error Response

```json
{
  "error": "Error message",
  "code": "ERROR_CODE"
}
```

### Common Error Codes
| Code | Description |
|------|-------------|
| INVALID_TOKEN | Token invalid or expired |
| ACCOUNT_EXPIRED | Account has expired |
| INVALID_REGISTRATION_CODE | Registration code invalid |
| DATABASE_NOT_FOUND | Database does not exist |
| NOTE_NOT_FOUND | Note does not exist |
| NAV_NOT_FOUND | Node does not exist |
