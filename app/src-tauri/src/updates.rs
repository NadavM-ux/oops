//! "A new version is available" notice. One small HTTPS request to GitHub's public API,
//! a few seconds after launch and then every 12 hours. Can be turned off in settings.

use serde::Serialize;
use std::time::Duration;

pub const REPO: &str = "NadavM-ux/oops";
pub const DOWNLOAD_PAGE: &str = "https://nadavm-ux.github.io/oops/#download";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub version: String,
    pub url: String,
}

pub fn check(current: &str) -> Option<Update> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .new_agent();
    let mut response = agent
        .get(&format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("User-Agent", "Oops-update-check")
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?;
    let release: serde_json::Value = response.body_mut().read_json().ok()?;
    let latest = release.get("tag_name")?.as_str()?.trim_start_matches('v').to_string();
    is_newer(&latest, current).then(|| Update { version: latest, url: DOWNLOAD_PAGE.into() })
}

fn parse(v: &str) -> (u64, u64, u64) {
    let mut it = v.split(['.', '-', '+']).map(|p| p.parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse(candidate) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_versions() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
    }
}
