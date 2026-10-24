use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn patch_documents(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            patch_documents(&path, found);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "needs OCTESSERA_STORE_COPY pointing at copied board stores"]
fn copied_board_stores_compose() {
    let root = PathBuf::from(
        std::env::var("OCTESSERA_STORE_COPY")
            .expect("set OCTESSERA_STORE_COPY to the copied stores"),
    );
    let mut checked = 0;
    let mut failures = Vec::new();
    for board in fs::read_dir(&root).unwrap() {
        let board = board.unwrap().path();
        let system_path = board.join("system.json");
        if !system_path.is_file() {
            continue;
        }
        let system: Value =
            serde_json::from_str(&fs::read_to_string(&system_path).unwrap()).unwrap();
        let mut documents = Vec::new();
        patch_documents(&board, &mut documents);
        for path in documents {
            let document: Value =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            if document["kind"] != "octessera.patch" {
                continue;
            }
            checked += 1;
            if let Err(error) =
                playback_runtime::compose_local_system_patch_documents(&system, &document)
            {
                failures.push(format!("{}: {error}", path.display()));
            }
        }
    }
    assert!(
        checked > 0,
        "no patch documents found under {}",
        root.display()
    );
    assert!(
        failures.is_empty(),
        "{checked} checked:\n{}",
        failures.join("\n")
    );
    println!("{checked} copied store documents compose");
}
