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

### Get Database List
```http
POST /hulunote/get-database-list
Authorization: Bearer <token>
Content-Type: application/json

Response:
{
  "databases": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "My Notebook",
      "description": "My personal notes",
      "created_at": "2026-02-08T10:00:00Z",
      "updated_at": "2026-02-08T10:00:00Z"
    }
  ]
}
```

### Create Database
```http
POST /hulunote/new-database
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "name": "New Database",
  "description": "Description here"
}

Response:
{
  "database": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "New Database",
    "description": "Description here",
    "created_at": "2026-02-08T10:00:00Z",
    "updated_at": "2026-02-08T10:00:00Z"
  }
}
```

### Update Database
```http
POST /hulunote/update-database
Authorization: Bearer <token>
Content-Type: application/json

Request:
{
  "database_id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Updated Name"
}

Response:
{
  "success": true
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

Request:
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
