mod api;
mod app;
mod components;
mod editor;
mod models;
mod pages;
mod state;
mod storage;
mod util;
mod wiki;

use leptos::prelude::*;

// Needed for `#[wasm_bindgen(start)]` on the wasm entrypoint.
#[cfg(all(target_arch = "wasm32", not(test)))]
use wasm_bindgen::prelude::wasm_bindgen;

// WASM-only tests (run with `cargo test --target wasm32-unknown-unknown` + wasm-bindgen-test-runner)
#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use crate::api::ApiClient;
    use crate::models::AccountInfo;
    use crate::storage::{load_user_from_storage, save_user_to_storage};
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_api_client_storage_roundtrip_token() {
        ApiClient::clear_storage();

        let mut c = ApiClient::load_from_storage();
        assert!(!c.is_authenticated());

        c.set_token("t1".to_string());
        c.save_to_storage();

        let c2 = ApiClient::load_from_storage();
        assert_eq!(c2.get_auth_token().as_deref(), Some("t1"));

        ApiClient::clear_storage();
        let c3 = ApiClient::load_from_storage();
        assert!(c3.get_auth_token().is_none());
    }

    #[wasm_bindgen_test]
    fn test_user_storage_roundtrip() {
        let user = AccountInfo {
            extra: serde_json::json!({"id": 1, "username": "u"}),
        };
        save_user_to_storage(&user);
        let loaded = load_user_from_storage().expect("should load user from localStorage");
        assert_eq!(loaded.extra["username"], "u");
    }
}

// Only register the WASM start function for normal builds (not for tests),
// otherwise wasm-bindgen-test will end up with multiple entry symbols.
#[cfg_attr(all(target_arch = "wasm32", not(test)), wasm_bindgen(start))]
pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(app::App);
}

#[cfg(test)]
mod tests {
    use crate::api::{ApiClient, LoginResponse, SignupRequest, SignupResponse};
    use crate::editor::{
        apply_nav_content, backfill_content_request, compute_reorder_target, get_nav_content,
        is_tmp_nav_id, make_tmp_nav_id, swap_tmp_nav_id,
    };
    use crate::models::{Nav, Note, RecentDb, RecentNote};
    use crate::storage::upsert_lru_by_key;
    use crate::util::next_available_daily_note_title_for_date;

    #[test]
    fn test_login_response_contract_deserialize() {
        // Contract based on hulunote-rust: handlers/auth.rs
        let json = r#"{
            "token": "jwt-token",
            "hulunote": {"id": 1, "username": "u", "mail": "u@example.com"},
            "region": null
        }"#;
        let parsed: LoginResponse =
            serde_json::from_str(json).expect("login response should parse");
        assert_eq!(parsed.token, "jwt-token");
        // hulunote is opaque; just ensure it's an object
        assert!(parsed.hulunote.extra.is_object());
        assert!(parsed.region.is_none());
    }

    #[test]
    fn test_signup_response_contract_deserialize() {
        // Contract based on hulunote-rust: handlers/auth.rs
        let json = r#"{
            "token": "jwt-token",
            "hulunote": {"id": 1, "username": "u"},
            "database": "u-1234",
            "region": null
        }"#;
        let parsed: SignupResponse =
            serde_json::from_str(json).expect("signup response should parse");
        assert_eq!(parsed.token, "jwt-token");
        assert_eq!(parsed.database.as_deref(), Some("u-1234"));
        assert!(parsed.hulunote.extra.is_object());
    }

    #[test]
    fn test_signup_request_serialization_includes_registration_code() {
        let req = SignupRequest {
            email: "u@example.com".to_string(),
            username: "u".to_string(),
            password: "pass".to_string(),
            registration_code: "FA8E-AF6E-4578-9347".to_string(),
        };
        let v = serde_json::to_value(req).expect("should serialize");
        assert_eq!(v["email"], "u@example.com");
        assert_eq!(v["username"], "u");
        assert_eq!(v["registration-code"], "FA8E-AF6E-4578-9347");
    }

    #[test]
    fn test_api_client_new() {
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert_eq!(client.base_url, "http://localhost:6689");
        assert!(client.token.is_none());
    }

    #[test]
    fn test_api_client_set_token() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_token("test-token".to_string());
        assert_eq!(client.token, Some("test-token".to_string()));
    }

    #[test]
    fn test_api_client_get_auth_token_without_token() {
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert!(client.get_auth_token().is_none());
    }

    #[test]
    fn test_api_client_get_auth_token_with_token() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_token("my-jwt-token".to_string());
        let token = client.get_auth_token().expect("Should have auth token");
        assert_eq!(token, "my-jwt-token");
    }

    #[test]
    fn test_api_client_no_refresh_token_support() {
        // hulunote-rust does not expose refresh tokens.
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert!(client.get_auth_token().is_none());
    }

    #[test]
    fn test_api_client_is_authenticated_false() {
        let client = ApiClient::new("http://localhost:6689".to_string());
        assert!(!client.is_authenticated());
    }

    #[test]
    fn test_api_client_is_authenticated_true() {
        let mut client = ApiClient::new("http://localhost:6689".to_string());
        client.set_token("my-jwt-token".to_string());
        assert!(client.is_authenticated());
    }

    #[test]
    fn test_apply_nav_content_updates_matching_nav() {
        let mut navs = vec![
            Nav {
                id: "a".to_string(),
                note_id: "n".to_string(),
                parid: "root".to_string(),
                same_deep_order: 1.0,
                content: "old".to_string(),
                is_display: true,
                is_delete: false,
            },
            Nav {
                id: "b".to_string(),
                note_id: "n".to_string(),
                parid: "root".to_string(),
                same_deep_order: 2.0,
                content: "keep".to_string(),
                is_display: true,
                is_delete: false,
            },
        ];

        assert!(apply_nav_content(&mut navs, "a", "new"));
        assert_eq!(navs[0].content, "new");
        assert_eq!(navs[1].content, "keep");
    }

    #[test]
    fn test_apply_nav_content_returns_false_when_missing() {
        let mut navs = vec![Nav {
            id: "a".to_string(),
            note_id: "n".to_string(),
            parid: "root".to_string(),
            same_deep_order: 1.0,
            content: "old".to_string(),
            is_display: true,
            is_delete: false,
        }];

        assert!(!apply_nav_content(&mut navs, "missing", "new"));
        assert_eq!(navs[0].content, "old");
    }

    #[test]
    fn test_is_tmp_nav_id() {
        assert!(is_tmp_nav_id("tmp-1-2"));
        assert!(!is_tmp_nav_id("real"));
    }

    #[test]
    fn test_make_tmp_nav_id_is_deterministic() {
        assert_eq!(make_tmp_nav_id(123, 456), "tmp-123-456");
    }

    #[test]
    fn test_swap_tmp_nav_id_updates_id() {
        let mut navs = vec![Nav {
            id: "tmp-1-2".to_string(),
            note_id: "n".to_string(),
            parid: "root".to_string(),
            same_deep_order: 1.0,
            content: "".to_string(),
            is_display: true,
            is_delete: false,
        }];

        assert!(swap_tmp_nav_id(&mut navs, "tmp-1-2", "real"));
        assert_eq!(navs[0].id, "real");
    }

    #[test]
    fn test_swap_tmp_nav_id_returns_false_when_missing() {
        let mut navs = vec![Nav {
            id: "x".to_string(),
            note_id: "n".to_string(),
            parid: "root".to_string(),
            same_deep_order: 1.0,
            content: "".to_string(),
            is_display: true,
            is_delete: false,
        }];

        assert!(!swap_tmp_nav_id(&mut navs, "tmp-1-2", "real"));
        assert_eq!(navs[0].id, "x");
    }

    #[test]
    fn test_get_nav_content_returns_value() {
        let navs = vec![Nav {
            id: "a".to_string(),
            note_id: "n".to_string(),
            parid: "root".to_string(),
            same_deep_order: 1.0,
            content: "hello".to_string(),
            is_display: true,
            is_delete: false,
        }];

        assert_eq!(get_nav_content(&navs, "a"), Some("hello".to_string()));
        assert_eq!(get_nav_content(&navs, "missing"), None);
    }

    #[test]
    fn test_backfill_content_request_empty_skips() {
        assert!(backfill_content_request("n", "id", "").is_none());
        assert!(backfill_content_request("n", "id", "   ").is_none());
    }

    #[test]
    fn test_backfill_content_request_builds_req() {
        let req = backfill_content_request("n1", "id1", "hello")
            .expect("should build request for non-empty content");
        assert_eq!(req.note_id, "n1");
        assert_eq!(req.id.as_deref(), Some("id1"));
        assert_eq!(req.content.as_deref(), Some("hello"));
        assert!(req.parid.is_none());
        assert!(req.order.is_none());
    }

    #[test]
    fn test_compute_reorder_target_moves_across_parent_before_target() {
        let all = vec![
            Nav {
                id: "d".to_string(),
                note_id: "n".to_string(),
                parid: "p1".to_string(),
                same_deep_order: 10.0,
                content: "".to_string(),
                is_display: true,
                is_delete: false,
            },
            Nav {
                id: "t".to_string(),
                note_id: "n".to_string(),
                parid: "p2".to_string(),
                same_deep_order: 5.0,
                content: "".to_string(),
                is_display: true,
                is_delete: false,
            },
            Nav {
                id: "u".to_string(),
                note_id: "n".to_string(),
                parid: "p2".to_string(),
                same_deep_order: 9.0,
                content: "".to_string(),
                is_display: true,
                is_delete: false,
            },
        ];

        let (parid, order) =
            compute_reorder_target(&all, "d", "t", false).expect("should compute reorder target");
        assert_eq!(parid, "p2");
        assert!(order < 5.0);
    }

    #[test]
    fn test_compute_reorder_target_moves_within_parent_after_target_between() {
        let all = vec![
            Nav {
                id: "a".to_string(),
                note_id: "n".to_string(),
                parid: "p".to_string(),
                same_deep_order: 1.0,
                content: "".to_string(),
                is_display: true,
                is_delete: false,
            },
            Nav {
                id: "d".to_string(),
                note_id: "n".to_string(),
                parid: "p".to_string(),
                same_deep_order: 2.0,
                content: "".to_string(),
                is_display: true,
                is_delete: false,
            },
            Nav {
                id: "t".to_string(),
                note_id: "n".to_string(),
                parid: "p".to_string(),
                same_deep_order: 3.0,
                content: "".to_string(),
                is_display: true,
                is_delete: false,
            },
            Nav {
                id: "b".to_string(),
                note_id: "n".to_string(),
                parid: "p".to_string(),
                same_deep_order: 10.0,
                content: "".to_string(),
                is_display: true,
                is_delete: false,
            },
        ];

        let (parid, order) =
            compute_reorder_target(&all, "d", "t", true).expect("should compute reorder target");
        assert_eq!(parid, "p");
        assert!(order > 3.0 && order < 10.0);
    }

    // NOTE: database list parsing is intentionally strict to the canonical contract.
    // The canonical database list shape is covered by `test_parse_database_list_response_legacy_shape`.

    #[test]
    fn test_parse_database_list_response_legacy_shape() {
        let v = serde_json::json!({
            "database-list": [
                {
                    "hulunote-databases/id": "0a1dd8e1-e255-4b35-937e-bac27dea1274",
                    "hulunote-databases/name": "ypyf-9361",
                    "hulunote-databases/description": "",
                    "hulunote-databases/created-at": "2026-02-08T15:59:24.130460+00:00",
                    "hulunote-databases/updated-at": "2026-02-08T15:59:24.130460+00:00"
                }
            ],
            "settings": {}
        });

        let out = ApiClient::parse_database_list_response(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "ypyf-9361");
        assert!(out[0].id.starts_with("0a1dd8e1"));
    }

    // NOTE: note list parsing is intentionally strict to the canonical contract.
    // The canonical note list shape is covered by `test_parse_note_list_response_legacy_shape_note_list`.

    #[test]
    fn test_parse_note_list_response_legacy_shape_note_list() {
        let v = serde_json::json!({
            "note-list": [
                {
                    "hulunote-notes/id": "n2",
                    "hulunote-notes/database-id": "db2",
                    "hulunote-notes/title": "Legacy",
                    "hulunote-notes/created-at": "t1",
                    "hulunote-notes/updated-at": "t2"
                }
            ]
        });

        let out = ApiClient::parse_note_list_response(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "n2");
        assert_eq!(out[0].database_id, "db2");
        assert_eq!(out[0].title, "Legacy");
        assert_eq!(out[0].updated_at, "t2");
    }

    #[test]
    fn test_next_available_daily_note_title_adds_suffix() {
        let base = "20260209";

        let notes = vec![
            Note {
                id: "n1".to_string(),
                database_id: "db".to_string(),
                title: base.to_string(),
                content: "".to_string(),
                created_at: "t1".to_string(),
                updated_at: "t2".to_string(),
            },
            Note {
                id: "n2".to_string(),
                database_id: "db".to_string(),
                title: format!("{}-2", base),
                content: "".to_string(),
                created_at: "t1".to_string(),
                updated_at: "t2".to_string(),
            },
        ];

        let next = next_available_daily_note_title_for_date(base, &notes);
        assert_eq!(next, format!("{}-3", base));
    }

    #[test]
    fn test_upsert_lru_by_key_dedup_and_order() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let out = upsert_lru_by_key(items, "b".to_string(), |x, y| x == y, 10);
        assert_eq!(out, vec!["b", "a", "c"]);
    }

    #[test]
    fn test_upsert_lru_by_key_truncate() {
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let out = upsert_lru_by_key(items, "d".to_string(), |x, y| x == y, 3);
        assert_eq!(out, vec!["d", "a", "b"]);
    }

    #[test]
    fn test_recent_structs_serde_roundtrip() {
        let db = RecentDb {
            id: "db1".to_string(),
            name: "My DB".to_string(),
            last_opened_ms: 123,
        };
        let note = RecentNote {
            db_id: "db1".to_string(),
            note_id: "n1".to_string(),
            title: "T".to_string(),
            last_opened_ms: 456,
        };

        let db_json = serde_json::to_string(&db).unwrap();
        let db2: RecentDb = serde_json::from_str(&db_json).unwrap();
        assert_eq!(db, db2);

        let note_json = serde_json::to_string(&note).unwrap();
        let note2: RecentNote = serde_json::from_str(&note_json).unwrap();
        assert_eq!(note, note2);
    }
}
