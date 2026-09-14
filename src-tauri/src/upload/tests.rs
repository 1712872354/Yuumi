//! upload 模块纯函数单元测试。

use serde_json::json;

use super::trigger::extract_gameflow_game_id;
use super::url::{
    build_batch_upload_url, build_upload_url, ensure_scheme, should_trigger_upload_on_phase,
};

#[test]
fn extracts_game_id_from_gameflow_session() {
    let data = json!({
        "gameData": {
            "gameId": 300900190786u64
        }
    });

    assert_eq!(extract_gameflow_game_id(&data), Some(300900190786));
}

#[test]
fn ignores_missing_or_zero_gameflow_game_id() {
    assert_eq!(extract_gameflow_game_id(&json!({})), None);
    assert_eq!(
        extract_gameflow_game_id(&json!({ "gameData": { "gameId": 0 } })),
        None
    );
}

#[test]
fn ensure_scheme_defaults_to_http() {
    assert_eq!(ensure_scheme("example.com"), "http://example.com");
    assert_eq!(ensure_scheme("http://example.com"), "http://example.com");
    assert_eq!(ensure_scheme("https://example.com"), "https://example.com");
}

#[test]
fn build_upload_url_appends_path_once() {
    assert_eq!(
        build_upload_url("example.com"),
        "http://example.com/api/lol/upload"
    );
    assert_eq!(
        build_upload_url("http://example.com/"),
        "http://example.com/api/lol/upload"
    );
    // 已包含路径时不重复追加
    assert_eq!(
        build_upload_url("http://example.com/api/lol/upload"),
        "http://example.com/api/lol/upload"
    );
}

#[test]
fn build_batch_upload_url_replaces_single_path() {
    assert_eq!(
        build_batch_upload_url("example.com"),
        "http://example.com/api/lol/upload-batch"
    );
    assert_eq!(
        build_batch_upload_url("http://example.com/api/lol/upload"),
        "http://example.com/api/lol/upload-batch"
    );
    assert_eq!(
        build_batch_upload_url("https://example.com/"),
        "https://example.com/api/lol/upload-batch"
    );
}

#[test]
fn upload_triggers_only_from_in_game_to_end_phases() {
    // 正常结算
    assert!(should_trigger_upload_on_phase("EndOfGame", "InProgress"));
    assert!(should_trigger_upload_on_phase("Lobby", "GameStart"));
    assert!(should_trigger_upload_on_phase("None", "PreEndOfGame"));
    assert!(should_trigger_upload_on_phase("EndOfGame", "Reconnect"));

    // 非结束阶段
    assert!(!should_trigger_upload_on_phase("InProgress", "GameStart"));
    assert!(!should_trigger_upload_on_phase("ChampSelect", "Lobby"));

    // 未进入对局（秒退/拒绝匹配）不触发
    assert!(!should_trigger_upload_on_phase("Lobby", "ChampSelect"));
    assert!(!should_trigger_upload_on_phase("Lobby", "ReadyCheck"));
    assert!(!should_trigger_upload_on_phase("None", "None"));
}
