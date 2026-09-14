//! 路人集：SQLite 持久化、标记管理、相遇历史、导入导出。

pub mod commands;
mod db;
mod encounters;
pub mod import_export;
mod types;

pub use commands::{
    delete_saved_player, get_saved_players_map, query_all_saved_players, query_encountered_games,
    save_saved_player, set_player_list_kind, SaveSavedPlayerInput, SavedPlayerMarker,
};
pub use db::init_db;
pub use encounters::{apply_auto_tags, query_tagged_for_reminder, record_encounters};
pub use import_export::{
    backfill_saved_player_identity, export_tagged_players_to_json_file,
    import_tagged_players_from_json_file,
};
pub use types::{
    CurrentGameCache, EncounteredGameDto, GamePlayerEntry, PageResult, SavedPlayerDto,
};

pub(crate) use db::{now, with_db};
