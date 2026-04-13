use std::collections::BTreeMap;

pub(super) fn github_https_git_env() -> BTreeMap<String, String> {
    github_https_git_env_from(
        std::env::var("GITHUB_TOKEN").ok().as_deref(),
        std::env::var("GH_TOKEN").ok().as_deref(),
    )
}

fn github_https_git_env_from(
    github_token: Option<&str>,
    gh_token: Option<&str>,
) -> BTreeMap<String, String> {
    let token = github_token
        .filter(|value| !value.trim().is_empty())
        .or_else(|| gh_token.filter(|value| !value.trim().is_empty()));

    let Some(token) = token else {
        return BTreeMap::new();
    };

    BTreeMap::from([
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        (
            "GIT_CONFIG_KEY_0".to_string(),
            format!("url.https://x-access-token:{token}@github.com/.insteadOf"),
        ),
        (
            "GIT_CONFIG_VALUE_0".to_string(),
            "https://github.com/".to_string(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::github_https_git_env_from;

    #[test]
    fn github_https_git_env_is_empty_without_tokens() {
        let env = github_https_git_env_from(None, None);

        assert!(env.is_empty());
    }

    #[test]
    fn github_https_git_env_prefers_github_token() {
        let env = github_https_git_env_from(Some("github-token"), Some("gh-token"));

        assert_eq!(env.get("GIT_CONFIG_COUNT").map(String::as_str), Some("1"));
        assert_eq!(
            env.get("GIT_CONFIG_KEY_0").map(String::as_str),
            Some("url.https://x-access-token:github-token@github.com/.insteadOf")
        );
        assert_eq!(
            env.get("GIT_CONFIG_VALUE_0").map(String::as_str),
            Some("https://github.com/")
        );
    }
}
