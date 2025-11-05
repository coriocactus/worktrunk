//! Template expansion utilities for worktrunk
//!
//! Provides functions for expanding template variables in paths and commands with
//! proper shell escaping to prevent injection vulnerabilities.

/// Expand template variables in a string
///
/// All templates support:
/// - `{main-worktree}` - Main worktree directory name
/// - `{branch}` - Branch name (sanitized: slashes → dashes)
///
/// Additional variables can be provided via the `extra` parameter.
///
/// # Examples
/// ```
/// use worktrunk::config::expand_template;
/// use std::collections::HashMap;
///
/// let result = expand_template("path/{main-worktree}/{branch}", "myrepo", "feature/foo", &HashMap::new());
/// assert_eq!(result, "path/myrepo/feature-foo");
/// ```
pub fn expand_template(
    template: &str,
    main_worktree: &str,
    branch: &str,
    extra: &std::collections::HashMap<&str, &str>,
) -> String {
    use shell_escape::escape;
    use std::borrow::Cow;

    // Sanitize branch name by replacing path separators
    let safe_branch = branch.replace(['/', '\\'], "-");

    // Shell-escape all variables to prevent issues with spaces and special characters
    let escaped_worktree = escape(Cow::Borrowed(main_worktree));
    let escaped_branch = escape(Cow::Borrowed(safe_branch.as_str()));

    let mut result = template
        .replace("{main-worktree}", &escaped_worktree)
        .replace("{branch}", &escaped_branch);

    // Apply any extra variables (also escaped)
    for (key, value) in extra {
        let escaped_value = escape(Cow::Borrowed(*value));
        result = result.replace(&format!("{{{}}}", key), &escaped_value);
    }

    result
}

/// Expand command template variables
///
/// Convenience function for expanding command templates with common variables.
///
/// Supported variables:
/// - `{repo}` - Repository name
/// - `{branch}` - Branch name (sanitized)
/// - `{worktree}` - Path to the worktree
/// - `{repo_root}` - Path to the main repository root
/// - `{target}` - Target branch (for merge commands, optional)
///
/// # Examples
/// ```
/// use worktrunk::config::expand_command_template;
/// use std::path::Path;
///
/// let cmd = expand_command_template(
///     "cp {repo_root}/target {worktree}/target",
///     "myrepo",
///     "feature",
///     Path::new("/path/to/worktree"),
///     Path::new("/path/to/repo"),
///     None,
/// );
/// ```
pub fn expand_command_template(
    command: &str,
    repo_name: &str,
    branch: &str,
    worktree_path: &std::path::Path,
    repo_root: &std::path::Path,
    target_branch: Option<&str>,
) -> String {
    let mut extra = std::collections::HashMap::new();
    extra.insert("worktree", worktree_path.to_str().unwrap_or(""));
    extra.insert("repo_root", repo_root.to_str().unwrap_or(""));
    if let Some(target) = target_branch {
        extra.insert("target", target);
    }

    expand_template(command, repo_name, branch, &extra)
}
