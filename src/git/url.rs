//! Git remote URL parsing.
//!
//! Parses git remote URLs into structured components (host, owner, repo).
//! Supports HTTPS, SSH, and git@ URL formats.

/// Parsed git remote URL with host, owner, and repository components.
///
/// # Supported URL formats
///
/// - `https://<host>/<owner>/<repo>.git`
/// - `http://<host>/<owner>/<repo>.git`
/// - `git@<host>:<owner>/<repo>.git`
/// - `ssh://git@<host>/<owner>/<repo>.git`
/// - `ssh://<host>/<owner>/<repo>.git`
///
/// # Example
///
/// ```
/// use worktrunk::git::GitRemoteUrl;
///
/// let url = GitRemoteUrl::parse("git@github.com:owner/repo.git").unwrap();
/// assert_eq!(url.host(), "github.com");
/// assert_eq!(url.owner(), "owner");
/// assert_eq!(url.repo(), "repo");
/// assert_eq!(url.project_identifier(), "github.com/owner/repo");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRemoteUrl {
    host: String,
    owner: String,
    repo: String,
}

impl GitRemoteUrl {
    /// Parse a git remote URL into structured components.
    ///
    /// Returns `None` for malformed URLs or unsupported formats.
    pub fn parse(url: &str) -> Option<Self> {
        let url = url.trim();

        let (host, owner, repo_with_suffix) = if let Some(rest) = url.strip_prefix("https://") {
            // https://github.com/owner/repo.git
            let mut parts = rest.split('/');
            let host = parts.next()?;
            let owner = parts.next()?;
            let repo = parts.next()?;
            (host, owner, repo)
        } else if let Some(rest) = url.strip_prefix("http://") {
            // http://github.com/owner/repo.git
            let mut parts = rest.split('/');
            let host = parts.next()?;
            let owner = parts.next()?;
            let repo = parts.next()?;
            (host, owner, repo)
        } else if let Some(rest) = url.strip_prefix("ssh://") {
            // ssh://git@github.com/owner/repo.git or ssh://github.com/owner/repo.git
            // Note: URLs with ports (ssh://host:2222/...) are not supported here
            // as they don't fit the host/owner/repo model. They should be handled
            // as raw strings (project_identifier fallback).
            let without_user = rest.split('@').next_back()?;
            let mut parts = without_user.split('/');
            let host = parts.next()?;
            // If host contains a colon (port), this URL doesn't fit our model
            if host.contains(':') {
                return None;
            }
            let owner = parts.next()?;
            let repo = parts.next()?;
            (host, owner, repo)
        } else if let Some(rest) = url.strip_prefix("git@") {
            // git@github.com:owner/repo.git
            let (host, path) = rest.split_once(':')?;
            let mut parts = path.split('/');
            let owner = parts.next()?;
            let repo = parts.next()?;
            (host, owner, repo)
        } else {
            return None;
        };

        // Strip .git suffix from repo if present
        let repo = repo_with_suffix
            .strip_suffix(".git")
            .unwrap_or(repo_with_suffix);

        // Validate non-empty
        if host.is_empty() || owner.is_empty() || repo.is_empty() {
            return None;
        }

        Some(Self {
            host: host.to_string(),
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }

    /// The hostname (e.g., "github.com", "gitlab.example.com").
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The repository owner or organization (e.g., "owner", "company-org").
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The repository name without .git suffix (e.g., "repo").
    pub fn repo(&self) -> &str {
        &self.repo
    }

    /// Project identifier in "host/owner/repo" format.
    ///
    /// Used for tracking approved commands per project.
    pub fn project_identifier(&self) -> String {
        format!("{}/{}/{}", self.host, self.owner, self.repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_https_urls() {
        let url = GitRemoteUrl::parse("https://github.com/owner/repo.git").unwrap();
        assert_eq!(url.host(), "github.com");
        assert_eq!(url.owner(), "owner");
        assert_eq!(url.repo(), "repo");
        assert_eq!(url.project_identifier(), "github.com/owner/repo");

        // Without .git suffix
        let url = GitRemoteUrl::parse("https://github.com/owner/repo").unwrap();
        assert_eq!(url.repo(), "repo");

        // With whitespace
        let url = GitRemoteUrl::parse("  https://github.com/owner/repo.git\n").unwrap();
        assert_eq!(url.owner(), "owner");
    }

    #[test]
    fn test_http_urls() {
        let url = GitRemoteUrl::parse("http://gitlab.internal.company.com/owner/repo.git").unwrap();
        assert_eq!(url.host(), "gitlab.internal.company.com");
        assert_eq!(url.owner(), "owner");
        assert_eq!(url.repo(), "repo");
    }

    #[test]
    fn test_git_at_urls() {
        let url = GitRemoteUrl::parse("git@github.com:owner/repo.git").unwrap();
        assert_eq!(url.host(), "github.com");
        assert_eq!(url.owner(), "owner");
        assert_eq!(url.repo(), "repo");

        // Without .git suffix
        let url = GitRemoteUrl::parse("git@github.com:owner/repo").unwrap();
        assert_eq!(url.repo(), "repo");

        // GitLab
        let url = GitRemoteUrl::parse("git@gitlab.example.com:owner/repo.git").unwrap();
        assert_eq!(url.host(), "gitlab.example.com");

        // Bitbucket
        let url = GitRemoteUrl::parse("git@bitbucket.org:owner/repo.git").unwrap();
        assert_eq!(url.host(), "bitbucket.org");
    }

    #[test]
    fn test_ssh_urls() {
        // With git@ user
        let url = GitRemoteUrl::parse("ssh://git@github.com/owner/repo.git").unwrap();
        assert_eq!(url.host(), "github.com");
        assert_eq!(url.owner(), "owner");
        assert_eq!(url.repo(), "repo");

        // Without user
        let url = GitRemoteUrl::parse("ssh://github.com/owner/repo.git").unwrap();
        assert_eq!(url.host(), "github.com");
        assert_eq!(url.owner(), "owner");
    }

    #[test]
    fn test_malformed_urls() {
        assert!(GitRemoteUrl::parse("").is_none());
        assert!(GitRemoteUrl::parse("https://github.com/").is_none());
        assert!(GitRemoteUrl::parse("https://github.com/owner/").is_none());
        assert!(GitRemoteUrl::parse("git@github.com:").is_none());
        assert!(GitRemoteUrl::parse("git@github.com:owner/").is_none());
        assert!(GitRemoteUrl::parse("ftp://github.com/owner/repo.git").is_none());
    }

    #[test]
    fn test_org_repos() {
        let url = GitRemoteUrl::parse("https://github.com/company-org/project.git").unwrap();
        assert_eq!(url.owner(), "company-org");
        assert_eq!(url.repo(), "project");
    }

    #[test]
    fn test_project_identifier() {
        let cases = [
            (
                "https://github.com/max-sixty/worktrunk.git",
                "github.com/max-sixty/worktrunk",
            ),
            ("git@github.com:owner/repo.git", "github.com/owner/repo"),
            (
                "ssh://git@gitlab.example.com/org/project.git",
                "gitlab.example.com/org/project",
            ),
        ];

        for (input, expected) in cases {
            let url = GitRemoteUrl::parse(input).unwrap();
            assert_eq!(url.project_identifier(), expected, "input: {input}");
        }
    }
}
