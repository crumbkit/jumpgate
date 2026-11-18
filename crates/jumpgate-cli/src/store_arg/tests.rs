use std::path::Path;

use super::*;

#[test]
fn the_personal_board_is_the_default_store() {
    let args = StoreArgs {
        store: None,
        board: None,
    };
    let chosen = args.open().unwrap();
    assert_eq!(chosen.store().board().as_str(), "personal");
    assert_eq!(chosen.place(), "the personal board");
}

#[test]
fn a_board_directory_gives_its_last_part_as_the_board_id() {
    let args = StoreArgs {
        store: None,
        board: Some(Path::new("/work/team-notes").to_path_buf()),
    };
    let chosen = args.open().unwrap();
    assert_eq!(chosen.store().board().as_str(), "team-notes");
    assert_eq!(chosen.place(), "the board in /work/team-notes");
}

#[test]
fn a_board_directory_that_no_board_id_names_is_refused() {
    let args = StoreArgs {
        store: None,
        board: Some(Path::new("/work/My Notes").to_path_buf()),
    };
    let error = args.open().err().unwrap();
    assert_eq!(error.reason(), Reason::BoardFile);
}

#[test]
fn a_store_directory_is_opened_as_a_directory_store() {
    let dir = tempfile::tempdir().unwrap();
    let args = StoreArgs {
        store: Some(dir.path().to_path_buf()),
        board: None,
    };
    let chosen = args.open().unwrap();
    assert_eq!(chosen.store().board().as_str(), "local");
}
