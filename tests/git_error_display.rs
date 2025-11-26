use insta::assert_snapshot;
use std::path::PathBuf;
use worktrunk::git::GitError;

#[test]
fn display_worktree_removal_failed() {
    let err = GitError::WorktreeRemovalFailed {
        branch: "feature-x".into(),
        path: PathBuf::from("/tmp/repo.feature-x"),
        error: "fatal: worktree is dirty\nerror: could not remove worktree".into(),
    };

    assert_snapshot!("worktree_removal_failed", err.styled());
}

#[test]
fn display_push_failed() {
    let err = GitError::PushFailed {
        error: "To /Users/user/workspace/repo/.git\n ! [remote rejected] HEAD -> main (Up-to-date check failed)\nerror: failed to push some refs to '/Users/user/workspace/repo/.git'".into(),
    };

    assert_snapshot!("push_failed", err.styled());
}
