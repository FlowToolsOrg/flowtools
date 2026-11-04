use super::*;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    directory: PathBuf,
    profile: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("flowtools-file-plan-{}", uuid::Uuid::new_v4()));
        let directory = root.join("selected");
        let profile = root.join("profile");
        fs::create_dir_all(&directory).unwrap();
        fs::create_dir(&profile).unwrap();
        Self {
            root,
            directory,
            profile,
        }
    }
    fn draft(&self) -> Result<RenameDraft, ErrorCode> {
        prepare_rename(&self.directory, "整理_", &self.profile)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.root.parent().unwrap(), std::env::temp_dir());
        assert!(self
            .root
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("flowtools-file-plan-"));
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn preview_is_read_only_bounded_and_rechecks_actual_bytes() {
    let fixture = Fixture::new();
    fs::write(fixture.directory.join("会议纪要.txt"), "会议内容").unwrap();
    fs::write(fixture.directory.join("整理_旧文档.txt"), "already").unwrap();
    fs::create_dir(fixture.directory.join("nested")).unwrap();
    fs::write(fixture.directory.join("nested/untouched.txt"), "nested").unwrap();
    let draft = fixture.draft().unwrap();
    let preview = draft.preview();
    assert_eq!(preview.files.len(), 1);
    assert_eq!(preview.skipped, 1);
    assert_eq!(preview.files[0].target, "整理_会议纪要.txt");
    assert_eq!(preview.total_bytes, 12);
    assert_eq!(preview.files[0].sha256.len(), 64);
    assert!(!fixture.directory.join(&preview.files[0].target).exists());
    assert_eq!(
        draft.recheck(&fixture.profile).unwrap().digest,
        preview.digest
    );
    fs::write(fixture.directory.join("会议纪要.txt"), "外部修改").unwrap();
    assert_eq!(
        draft.recheck(&fixture.profile).unwrap_err(),
        ErrorCode::RevisionConflict
    );
}

#[test]
fn all_targets_preflight_without_any_effect() {
    let fixture = Fixture::new();
    for name in ["a.txt", "b.txt"] {
        fs::write(fixture.directory.join(name), name).unwrap()
    }
    let draft = fixture.draft().unwrap();
    fs::write(fixture.directory.join("整理_b.txt"), "keep collision").unwrap();
    assert_eq!(
        draft.recheck(&fixture.profile).unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        fs::read(fixture.directory.join("整理_b.txt")).unwrap(),
        b"keep collision"
    );
    assert!(fixture.directory.join("a.txt").exists());
    assert!(!fixture.directory.join("整理_a.txt").exists());
}

#[test]
fn held_move_refuses_late_collision_and_writer_then_moves_exact_source() {
    let fixture = Fixture::new();
    fs::write(fixture.directory.join("a.txt"), b"keep").unwrap();
    let draft = fixture.draft().unwrap();
    let held = draft.preflight(&fixture.profile).unwrap();
    assert!(fs::write(fixture.directory.join("a.txt"), b"changed").is_err());
    assert!(fs::rename(
        fixture.directory.join("a.txt"),
        fixture.directory.join("other.txt")
    )
    .is_err());
    fs::write(fixture.directory.join("整理_a.txt"), b"collision").unwrap();
    assert_eq!(
        held.root
            .rename(&held.sources[0], "整理_a.txt")
            .unwrap_err(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        fs::read(fixture.directory.join("整理_a.txt")).unwrap(),
        b"collision"
    );
    fs::remove_file(fixture.directory.join("整理_a.txt")).unwrap();
    held.root.rename(&held.sources[0], "整理_a.txt").unwrap();
    assert!(!fixture.directory.join("a.txt").exists());
    assert_eq!(
        fs::read(fixture.directory.join("整理_a.txt")).unwrap(),
        b"keep"
    );
    drop(held);
    assert_eq!(
        draft.recheck(&fixture.profile).unwrap_err(),
        ErrorCode::RevisionConflict
    );
}

#[test]
fn hardlinks_profile_subtrees_and_unstable_identities_are_refused() {
    let fixture = Fixture::new();
    fs::write(fixture.directory.join("a"), b"data").unwrap();
    let draft = fixture.draft().unwrap();
    fs::hard_link(fixture.directory.join("a"), fixture.root.join("linked")).unwrap();
    assert_eq!(
        draft.recheck(&fixture.profile).unwrap_err(),
        ErrorCode::ScopeDenied
    );
    assert!(matches!(fixture.draft(), Err(ErrorCode::ScopeDenied)));
    assert!(matches!(
        prepare_rename(&fixture.profile, "x", &fixture.profile),
        Err(ErrorCode::ScopeDenied)
    ));
    fs::create_dir(fixture.profile.join("child")).unwrap();
    assert!(matches!(
        prepare_rename(&fixture.profile.join("child"), "x", &fixture.profile),
        Err(ErrorCode::ScopeDenied)
    ));
    fs::remove_file(fixture.root.join("linked")).unwrap();
    fs::rename(fixture.directory.join("a"), fixture.root.join("original")).unwrap();
    fs::write(fixture.directory.join("a"), b"data").unwrap();
    assert_eq!(
        draft.recheck(&fixture.profile).unwrap_err(),
        ErrorCode::RevisionConflict
    );
}

#[test]
fn names_paths_and_real_inventory_budgets_fail_closed() {
    let fixture = Fixture::new();
    for prefix in ["", "../", "a:b", "x\u{202e}"] {
        assert!(matches!(
            prepare_rename(&fixture.directory, prefix, &fixture.profile),
            Err(ErrorCode::InputInvalid)
        ));
    }
    for name in [
        "..",
        "NUL.txt",
        "COM1",
        "COM¹.txt",
        "a ",
        "x\u{2066}y",
        "a:b",
    ] {
        assert_eq!(
            windows::component(name).unwrap_err(),
            ErrorCode::InputInvalid
        );
    }
    for path in [
        r"relative",
        r"C:\",
        r"\\server\share\files",
        r"\\.\C:\files",
    ] {
        assert!(prepare_rename(Path::new(path), "x", &fixture.profile).is_err());
    }
    for index in 0..=MAX_FILES {
        fs::write(fixture.directory.join(format!("{index}.txt")), b"a").unwrap()
    }
    assert!(matches!(fixture.draft(), Err(ErrorCode::BudgetExceeded)));
    let file = fs::OpenOptions::new()
        .write(true)
        .open(fixture.directory.join("0.txt"))
        .unwrap();
    file.set_len(MAX_FILE_BYTES + 1).unwrap();
    drop(file);
    assert!(matches!(fixture.draft(), Err(ErrorCode::BudgetExceeded)));
    assert_eq!(
        fs::read_dir(&fixture.directory).unwrap().count(),
        MAX_FILES + 1
    );
}

#[test]
fn junction_directory_and_leaf_are_refused_without_reading_the_target() {
    let fixture = Fixture::new();
    let outside = fixture.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("canary"), b"UNCHANGED").unwrap();
    let junction = fixture.directory.join("redirect");
    let status = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&junction)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(fixture.draft().is_err());
    assert!(prepare_rename(&junction, "x", &fixture.profile).is_err());
    assert_eq!(fs::read(outside.join("canary")).unwrap(), b"UNCHANGED");
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
}

#[test]
fn directory_entries_also_count_towards_inventory_budget() {
    let fixture = Fixture::new();
    for index in 0..=MAX_ENTRIES {
        fs::create_dir(fixture.directory.join(index.to_string())).unwrap()
    }
    assert!(matches!(fixture.draft(), Err(ErrorCode::BudgetExceeded)));
}
