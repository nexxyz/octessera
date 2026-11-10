use super::*;

#[test]
fn restore_writes_only_into_its_staging_folder() {
    let fixture = Fixture::new("staging-only");
    let names = |path: &Path| {
        let mut names: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let mut before = names(&fixture.root);
    before.push(STAGING_DIR_NAME.to_string());
    before.sort();

    fixture.restore("staging-only", true).unwrap();

    assert_eq!(names(&fixture.root), before);
    assert_eq!(
        names(&fixture.staging),
        ["octessera-pre-restore-staging-only.oct"]
    );
}

#[test]
fn a_restore_keeps_only_the_two_newest_pre_restore_archives() {
    let fixture = Fixture::new("archive-prune");
    fs::create_dir_all(&fixture.staging).unwrap();
    let now = std::time::SystemTime::now();
    for (age_hours, session) in [(3, "oldest"), (2, "older"), (1, "old")] {
        let path = fixture
            .staging
            .join(format!("octessera-pre-restore-{session}.oct"));
        fs::write(&path, b"archive").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(now - std::time::Duration::from_secs(age_hours * 3600))
            .unwrap();
    }
    fs::write(fixture.staging.join("notes.txt"), b"not an archive").unwrap();

    fixture.restore("newest", true).unwrap();

    let mut names: Vec<_> = fs::read_dir(&fixture.staging)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "notes.txt",
            "octessera-pre-restore-newest.oct",
            "octessera-pre-restore-old.oct"
        ]
    );
}
