use crate::common::TestRepo;

#[test]
fn test_get_default_branch_with_origin_head() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // origin/HEAD should be set automatically by setup_remote
    assert!(repo.has_origin_head());

    // Test that we can get the default branch
    let branch = worktrunk::git::get_default_branch_in(repo.root_path())
        .expect("Failed to get default branch");
    assert_eq!(branch, "main");
}

#[test]
fn test_get_default_branch_without_origin_head() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Clear origin/HEAD to force remote query
    repo.clear_origin_head();
    assert!(!repo.has_origin_head());

    // Should still work by querying remote
    let branch = worktrunk::git::get_default_branch_in(repo.root_path())
        .expect("Failed to get default branch");
    assert_eq!(branch, "main");

    // Verify that origin/HEAD is now cached
    assert!(repo.has_origin_head());
}

#[test]
fn test_get_default_branch_caches_result() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Clear origin/HEAD
    repo.clear_origin_head();
    assert!(!repo.has_origin_head());

    // First call queries remote and caches
    worktrunk::git::get_default_branch_in(repo.root_path()).expect("Failed to get default branch");
    assert!(repo.has_origin_head());

    // Second call uses cache (fast path)
    let branch = worktrunk::git::get_default_branch_in(repo.root_path())
        .expect("Failed to get default branch on second call");
    assert_eq!(branch, "main");
}

#[test]
fn test_get_default_branch_no_remote() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // No remote configured, should fail
    let result = worktrunk::git::get_default_branch_in(repo.root_path());
    assert!(result.is_err());
}
