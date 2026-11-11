use crate::support::{make_temp_dir, repo, write};
use claude_sessions_core::git_branch::{GitBranchReader, current};
use std::fs;
use std::time::{Duration, SystemTime};

#[test]
fn git_branch_plain_repo() {
    let d = repo("ref: refs/heads/bugfix/T6IR-31\n");
    assert_eq!(
        current(d.path().to_str().unwrap()).as_deref(),
        Some("bugfix/T6IR-31")
    );
}

#[test]
fn git_branch_from_subdirectory() {
    let d = repo("ref: refs/heads/bugfix/T6IR-31\n");
    let sub = d.path().join("a/b");
    fs::create_dir_all(&sub).unwrap();
    assert_eq!(
        current(sub.to_str().unwrap()).as_deref(),
        Some("bugfix/T6IR-31")
    );
}

#[test]
fn git_branch_worktree_relative_gitdir() {
    let root = make_temp_dir();
    write(
        &root.path().join("main/.git/worktrees/w/HEAD"),
        "ref: refs/heads/feature/x\n",
    );
    write(
        &root.path().join("w/.git"),
        "gitdir: ../main/.git/worktrees/w\n",
    );
    assert_eq!(
        current(root.path().join("w").to_str().unwrap()).as_deref(),
        Some("feature/x")
    );
}

#[test]
fn git_branch_worktree_subdir_relative_gitdir() {
    let root = make_temp_dir();
    write(
        &root.path().join("main/.git/worktrees/w/HEAD"),
        "ref: refs/heads/feature/x\n",
    );
    write(
        &root.path().join("w/.git"),
        "gitdir: ../main/.git/worktrees/w\n",
    );
    let sub = root.path().join("w/src");
    fs::create_dir_all(&sub).unwrap();
    assert_eq!(current(sub.to_str().unwrap()).as_deref(), Some("feature/x"));
}

#[test]
fn git_branch_worktree_absolute_gitdir() {
    let root = make_temp_dir();
    let gitdir = root.path().join("main/.git/worktrees/w");
    write(&gitdir.join("HEAD"), "ref: refs/heads/feature/x\n");
    write(
        &root.path().join("w/.git"),
        &format!("gitdir: {}\n", gitdir.display()),
    );
    assert_eq!(
        current(root.path().join("w").to_str().unwrap()).as_deref(),
        Some("feature/x")
    );
}

#[test]
fn git_branch_detached_is_nil() {
    let d = repo(&format!("{}\n", "a".repeat(40)));
    assert_eq!(current(d.path().to_str().unwrap()), None);
}

#[test]
fn git_branch_no_repo_is_nil() {
    assert_eq!(current(make_temp_dir().path().to_str().unwrap()), None);
}

#[test]
fn cached_branch_sees_checkout() {
    let d = repo("ref: refs/heads/main\n");
    let cwd = d.path().to_str().unwrap();
    let mut reader = GitBranchReader::new();
    assert_eq!(reader.current(cwd).as_deref(), Some("main"));
    assert_eq!(reader.current(cwd).as_deref(), Some("main"));
    assert_eq!(reader.head_reads(), 1);
    let head = d.path().join(".git/HEAD");
    fs::write(&head, "ref: refs/heads/feat\n").unwrap();
    let later = SystemTime::now() + Duration::from_secs(5);
    fs::File::options()
        .write(true)
        .open(&head)
        .unwrap()
        .set_modified(later)
        .unwrap();
    assert_eq!(reader.current(cwd).as_deref(), Some("feat"));
    fs::remove_dir_all(d.path().join(".git")).unwrap();
    assert_eq!(reader.current(cwd), None);
}
