use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Benchmark configuration profiles representing different repo sizes
struct BenchmarkProfile {
    name: &'static str,
    commits: usize,
    files: usize,
    commits_ahead: usize,
    commits_behind: usize,
    uncommitted_files: usize,
}

const PROFILES: &[BenchmarkProfile] = &[
    BenchmarkProfile {
        name: "minimal",
        commits: 10,
        files: 10,
        commits_ahead: 0,
        commits_behind: 0, // Skip for now - causes git checkout issues
        uncommitted_files: 0,
    },
    BenchmarkProfile {
        name: "typical",
        commits: 500,
        files: 100,
        commits_ahead: 10,
        commits_behind: 0, // Skip for now - causes git checkout issues
        uncommitted_files: 3,
    },
    BenchmarkProfile {
        name: "large",
        commits: 1000,
        files: 200,
        commits_ahead: 50,
        commits_behind: 0, // Skip for now - causes git checkout issues
        uncommitted_files: 10,
    },
];

fn run_git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Git command failed: {:?}\nstderr: {}\nstdout: {}\npath: {}",
        args,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
        path.display()
    );
}

/// Create a realistic repository with actual commit history and file changes
fn create_realistic_repo(commits: usize, files: usize) -> TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo_path = temp_dir.path().join("main");
    std::fs::create_dir(&repo_path).unwrap();

    // Initialize repository
    run_git(&repo_path, &["init", "-b", "main"]);
    run_git(&repo_path, &["config", "user.name", "Benchmark"]);
    run_git(&repo_path, &["config", "user.email", "bench@test.com"]);

    // Create initial file structure
    for i in 0..files {
        let file_path = repo_path.join(format!("src/file_{}.rs", i));
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        let content = format!(
            "// File {}\n\
             pub struct Module{} {{\n\
                 data: Vec<String>,\n\
             }}\n\n\
             pub fn function_{}() -> i32 {{\n\
                 {}\n\
             }}\n",
            i,
            i,
            i,
            i * 42
        );
        std::fs::write(&file_path, content).unwrap();
    }

    run_git(&repo_path, &["add", "."]);
    run_git(&repo_path, &["commit", "-m", "Initial commit"]);

    // Build commit history with realistic diffs
    for i in 1..commits {
        // Modify 2-3 files per commit for realistic git operations
        let num_files_to_modify = 2 + (i % 2);
        for j in 0..num_files_to_modify {
            let file_idx = (i * 7 + j * 13) % files; // Pseudo-random file selection
            let file_path = repo_path.join(format!("src/file_{}.rs", file_idx));
            let mut content = std::fs::read_to_string(&file_path).unwrap();
            content.push_str(&format!(
                "\npub fn function_{}_{}() -> i32 {{\n    {}\n}}\n",
                file_idx,
                i,
                i * 100 + j
            ));
            std::fs::write(&file_path, content).unwrap();
        }

        run_git(&repo_path, &["add", "."]);
        run_git(&repo_path, &["commit", "-m", &format!("Commit {}", i)]);
    }

    temp_dir
}

/// Add a worktree with diverged branch and uncommitted changes
fn add_worktree_with_divergence(
    temp_dir: &TempDir,
    repo_path: &Path,
    wt_num: usize,
    commits_ahead: usize,
    commits_behind: usize,
    uncommitted_files: usize,
) {
    let branch = format!("feature-{}", wt_num);
    let wt_path = temp_dir.path().join(format!("wt-{}", wt_num));

    // Get current HEAD to diverge from
    let head_output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    let base_commit = String::from_utf8_lossy(&head_output.stdout)
        .trim()
        .to_string();

    // Create worktree at current HEAD
    run_git(
        repo_path,
        &[
            "worktree",
            "add",
            "-b",
            &branch,
            wt_path.to_str().unwrap(),
            &base_commit,
        ],
    );

    // Add diverging commits in worktree (creates "ahead" status)
    for i in 0..commits_ahead {
        let file_path = wt_path.join(format!("feature_{}_file_{}.txt", wt_num, i));
        let content = format!(
            "Feature {} content {}\n\
             This is a realistic file with multiple lines\n\
             to make git diff operations non-trivial.\n",
            wt_num, i
        );
        std::fs::write(&file_path, content).unwrap();
        run_git(&wt_path, &["add", "."]);
        run_git(
            &wt_path,
            &["commit", "-m", &format!("Feature {} commit {}", wt_num, i)],
        );
    }

    // Add uncommitted changes (exercises git diff HEAD)
    for i in 0..uncommitted_files {
        let file_path = wt_path.join(format!("uncommitted_{}.txt", i));
        std::fs::write(&file_path, "Uncommitted content\n").unwrap();
    }

    // Add commits to main branch (creates "behind" status for worktree)
    if commits_behind > 0 {
        // Ensure we're on the main branch
        run_git(repo_path, &["checkout", "main"]);

        for i in 0..commits_behind {
            let file_path = repo_path.join(format!("main_advance_{}.txt", i));
            std::fs::write(&file_path, format!("Main content {}\n", i)).unwrap();
            run_git(repo_path, &["add", "."]);
            run_git(repo_path, &["commit", "-m", &format!("Main advance {}", i)]);
        }
    }
}

fn bench_list_by_worktree_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_by_worktree_count");

    // Build release binary once
    let build_output = Command::new("cargo")
        .args(&["build", "--release"])
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "Failed to build release binary"
    );

    let binary = std::env::current_dir().unwrap().join("target/release/wt");

    // Use "typical" profile for this benchmark
    let profile = &PROFILES[1];

    // Test with different worktree counts to find crossover point
    for num_worktrees in [1, 2, 3, 4, 6, 8] {
        // Setup repo ONCE per worktree count (wt list is read-only, so reuse is safe)
        let temp = create_realistic_repo(profile.commits, profile.files);
        let repo_path = temp.path().join("main");

        // Add worktrees with divergence
        for i in 1..num_worktrees {
            add_worktree_with_divergence(
                &temp,
                &repo_path,
                i,
                profile.commits_ahead,
                profile.commits_behind,
                profile.uncommitted_files,
            );
        }

        // Warm up git's internal caches
        run_git(&repo_path, &["status"]);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_worktrees),
            &num_worktrees,
            |b, _| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .current_dir(&repo_path)
                        .output()
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_list_by_repo_profile(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_by_profile");

    let binary = std::env::current_dir().unwrap().join("target/release/wt");

    // Fixed worktree count to isolate repo size impact
    let num_worktrees = 4;

    for profile in PROFILES {
        // Setup repo ONCE per profile (wt list is read-only)
        let temp = create_realistic_repo(profile.commits, profile.files);
        let repo_path = temp.path().join("main");

        for i in 1..num_worktrees {
            add_worktree_with_divergence(
                &temp,
                &repo_path,
                i,
                profile.commits_ahead,
                profile.commits_behind,
                profile.uncommitted_files,
            );
        }

        run_git(&repo_path, &["status"]);

        group.bench_with_input(
            BenchmarkId::from_parameter(profile.name),
            profile,
            |b, _profile| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .current_dir(&repo_path)
                        .output()
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

fn bench_sequential_vs_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_vs_parallel");

    // Build release binary once
    let build_output = Command::new("cargo")
        .args(&["build", "--release"])
        .output()
        .unwrap();
    assert!(
        build_output.status.success(),
        "Failed to build release binary"
    );

    let binary = std::env::current_dir().unwrap().join("target/release/wt");

    let profile = &PROFILES[1]; // typical profile

    // Test both sequential and parallel implementations across different worktree counts
    for num_worktrees in [1, 2, 3, 4, 6, 8] {
        let temp = create_realistic_repo(profile.commits, profile.files);
        let repo_path = temp.path().join("main");

        for i in 1..num_worktrees {
            add_worktree_with_divergence(
                &temp,
                &repo_path,
                i,
                profile.commits_ahead,
                profile.commits_behind,
                profile.uncommitted_files,
            );
        }

        run_git(&repo_path, &["status"]);

        // Benchmark parallel implementation (default)
        group.bench_with_input(
            BenchmarkId::new("parallel", num_worktrees),
            &num_worktrees,
            |b, _| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .current_dir(&repo_path)
                        .output()
                        .unwrap();
                });
            },
        );

        // Benchmark sequential implementation (via WT_SEQUENTIAL env var)
        group.bench_with_input(
            BenchmarkId::new("sequential", num_worktrees),
            &num_worktrees,
            |b, _| {
                b.iter(|| {
                    Command::new(&binary)
                        .arg("list")
                        .env("WT_SEQUENTIAL", "1")
                        .current_dir(&repo_path)
                        .output()
                        .unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(30)
        .measurement_time(std::time::Duration::from_secs(15))
        .warm_up_time(std::time::Duration::from_secs(3));
    targets = bench_list_by_worktree_count, bench_list_by_repo_profile, bench_sequential_vs_parallel
}
criterion_main!(benches);
