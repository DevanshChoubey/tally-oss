//! The opt-in update check (round s23).
//!
//! Off unless the user turns it on (the app keeps that choice). When asked, it makes these
//! requests and no others:
//!
//! 1. `GET` [`RELEASES_LATEST_URL`], the GitHub releases API for this project only;
//! 2. `GET` the release's `SHA256SUMS.txt`, from GitHub's release download hosts
//!    ([`DOWNLOAD_ORIGINS`]). Each redirect is checked by hand, and a redirect to any other host is
//!    not followed.
//!
//! Every request carries the generic user agent [`USER_AGENT`] and nothing else about the user:
//! no cookie, no token, no query string, no version number.
//!
//! A release is offered only when two independent records of every file agree: the `digest` the
//! API gives for each asset, and that asset's line in `SHA256SUMS.txt`, whose own bytes must
//! hash to the `digest` the API gives for it. Anything else is [`UpdateCheck::ChecksumsDisagree`]
//! or [`UpdateCheck::CouldNotCheck`], never an offer. Nothing is ever downloaded except that text
//! file, and nothing is installed. The user downloads the installer from the release page and can
//! compare its hash with [`sha256_of_file`].

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};
use ureq::Agent;

/// The only API address the default checker asks: the latest release of this project.
pub const RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/DevanshChoubey/tally-oss/releases/latest";

/// The hosts release files may be read from: GitHub, and the asset hosts its download links
/// redirect to (`release-assets.githubusercontent.com` was the redirect target on 2026-11-27;
/// `objects.githubusercontent.com` is the older one).
pub const DOWNLOAD_ORIGINS: [&str; 3] = [
    "https://github.com",
    "https://release-assets.githubusercontent.com",
    "https://objects.githubusercontent.com",
];

/// The name of the checksum file the release job writes.
pub const SUMS_NAME: &str = "SHA256SUMS.txt";

/// The user agent of every request. GitHub's API refuses requests without one; it names no user,
/// machine or version.
pub const USER_AGENT: &str = "Ledgerly-update-check";

/// Seconds the whole check may take.
const TIMEOUT_SECS: u64 = 20;

/// Largest reply read, in bytes. A release reply and a sums file are a few kilobytes.
const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Most redirects followed for one download.
const MAX_HOPS: usize = 4;

/// One file of a release, with the SHA-256 both records agree on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    /// The file name, for example `ledgerly-1.1.0-windows-x86_64-setup.exe`.
    pub name: String,
    /// SHA-256 of the file, lower-case hex.
    pub sha256: String,
    /// Where the user can download it (not fetched by the check).
    pub url: String,
}

/// A newer release whose checksums agree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// The tag, for example `v1.1.0`.
    pub tag: String,
    /// The version from the tag, for example `1.1.0`.
    pub version: String,
    /// The release page.
    pub page_url: String,
    /// Every file but the sums file.
    pub assets: Vec<ReleaseAsset>,
}

/// What the check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheck {
    /// The latest release is this version or older.
    UpToDate {
        /// The latest release's version.
        latest: String,
    },
    /// A newer release, with checksums that agree.
    Available(Release),
    /// The release's checksum records do not agree: it is not offered.
    ChecksumsDisagree {
        /// Which file disagrees, and how.
        reason: String,
    },
    /// The check did not complete (no network, a refused or rate-limited request, an unexpected
    /// reply, a redirect to another host).
    CouldNotCheck {
        /// What went wrong.
        reason: String,
    },
}

/// Runs the update check.
#[derive(Debug, Clone)]
pub struct UpdateChecker {
    /// The latest-release API address.
    api_url: String,
    /// Origins (`scheme://host[:port]`) release files may be read from.
    download_origins: Vec<String>,
}

impl UpdateChecker {
    /// The checker the app uses: [`RELEASES_LATEST_URL`] and [`DOWNLOAD_ORIGINS`].
    #[must_use]
    pub fn github() -> Self {
        Self::with_endpoints(RELEASES_LATEST_URL, &DOWNLOAD_ORIGINS)
    }

    /// A checker that asks `api_url` and reads release files only from `download_origins`. Tests
    /// point it at a local server; the app uses [`UpdateChecker::github`].
    #[must_use]
    pub fn with_endpoints(api_url: &str, download_origins: &[&str]) -> Self {
        Self {
            api_url: api_url.to_owned(),
            download_origins: download_origins.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    /// Asks for the latest release and compares it with `current` (this build's version).
    #[must_use]
    pub fn check(&self, current: &str) -> UpdateCheck {
        match self.run(current) {
            Ok(found) => found,
            Err(reason) => UpdateCheck::CouldNotCheck { reason },
        }
    }

    fn run(&self, current: &str) -> Result<UpdateCheck, String> {
        let agent = agent();
        let reply = get(&agent, &self.api_url)?;
        let json: serde_json::Value = serde_json::from_slice(&reply)
            .map_err(|e| format!("the release reply is not JSON: {e}"))?;
        let tag = text(&json, "tag_name")?;
        let version = tag.strip_prefix('v').unwrap_or(&tag).to_owned();
        let (Some(latest), Some(this)) = (parse_version(&version), parse_version(current)) else {
            return Err(format!(
                "the release tag '{tag}' or this version '{current}' is not a version number"
            ));
        };
        if latest <= this {
            return Ok(UpdateCheck::UpToDate { latest: version });
        }
        let page_url = text(&json, "html_url")?;
        let assets = assets(&json)?;
        let Some(sums) = assets.iter().find(|a| a.name == SUMS_NAME) else {
            return Err(format!("the release {tag} has no {SUMS_NAME}"));
        };
        let body = self.download(&agent, &sums.url)?;
        let actual = to_hex(&Sha256::digest(&body));
        if actual != sums.sha256 {
            return Ok(UpdateCheck::ChecksumsDisagree {
                reason: format!(
                    "{SUMS_NAME} hashes to {actual}, but the release lists {}",
                    sums.sha256
                ),
            });
        }
        let lines = parse_sums(&body)?;
        let mut offered = Vec::new();
        for asset in assets.into_iter().filter(|a| a.name != SUMS_NAME) {
            match lines.iter().find(|(_, name)| *name == asset.name) {
                Some((hash, _)) if *hash == asset.sha256 => offered.push(asset),
                Some((hash, _)) => {
                    return Ok(UpdateCheck::ChecksumsDisagree {
                        reason: format!(
                            "{} is {hash} in {SUMS_NAME} but {} in the release",
                            asset.name, asset.sha256
                        ),
                    });
                }
                None => {
                    return Ok(UpdateCheck::ChecksumsDisagree {
                        reason: format!("{} has no line in {SUMS_NAME}", asset.name),
                    });
                }
            }
        }
        Ok(UpdateCheck::Available(Release {
            tag,
            version,
            page_url,
            assets: offered,
        }))
    }

    /// Reads `url`, following at most [`MAX_HOPS`] redirects, each to an allowed origin.
    fn download(&self, agent: &Agent, url: &str) -> Result<Vec<u8>, String> {
        let mut at = url.to_owned();
        for _ in 0..=MAX_HOPS {
            let origin = origin_of(&at).ok_or_else(|| format!("'{at}' is not a web address"))?;
            if !self.download_origins.contains(&origin) {
                return Err(format!(
                    "the release file is at {origin}, which is not a GitHub download host; it was not read"
                ));
            }
            match fetch(agent, &at)? {
                Fetched::Body(body) => return Ok(body),
                Fetched::Redirect(next) => at = next,
            }
        }
        Err(format!("more than {MAX_HOPS} redirects for {url}"))
    }
}

/// The SHA-256 of the file at `path`, lower-case hex, for comparing a downloaded installer with
/// [`ReleaseAsset::sha256`].
///
/// # Errors
/// The file cannot be read.
pub fn sha256_of_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(buf.get(..n).unwrap_or_default());
    }
    Ok(to_hex(&hasher.finalize()))
}

fn agent() -> Agent {
    let config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(TIMEOUT_SECS)))
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(USER_AGENT)
        .build();
    Agent::new_with_config(config)
}

/// What one request returned.
enum Fetched {
    /// A 200 reply's body.
    Body(Vec<u8>),
    /// A redirect's target.
    Redirect(String),
}

fn fetch(agent: &Agent, url: &str) -> Result<Fetched, String> {
    let mut response = agent
        .get(url)
        .header(
            "Accept",
            "application/vnd.github+json, application/octet-stream",
        )
        .call()
        .map_err(|e| {
            format!(
                "could not reach {}: {e}",
                origin_of(url).unwrap_or_default()
            )
        })?;
    let status = response.status().as_u16();
    if (300..400).contains(&status) {
        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| format!("a {status} redirect with no address"))?;
        return Ok(Fetched::Redirect(location.to_owned()));
    }
    if status != 200 {
        let why = match status {
            403 => " (refused, or the hourly limit of the GitHub API was reached)",
            404 => " (no release found)",
            429 => " (too many requests; try later)",
            _ => "",
        };
        return Err(format!("the server answered {status}{why}"));
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .map(Fetched::Body)
        .map_err(|e| format!("the reply could not be read: {e}"))
}

fn get(agent: &Agent, url: &str) -> Result<Vec<u8>, String> {
    match fetch(agent, url)? {
        Fetched::Body(body) => Ok(body),
        Fetched::Redirect(to) => Err(format!("the release API redirected to {to}; not followed")),
    }
}

fn text(json: &serde_json::Value, key: &str) -> Result<String, String> {
    json.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("the release reply has no '{key}'"))
}

fn assets(json: &serde_json::Value) -> Result<Vec<ReleaseAsset>, String> {
    let list = json
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "the release reply has no 'assets'".to_owned())?;
    list.iter()
        .map(|a| {
            let name = text(a, "name")?;
            let digest = text(a, "digest").map_err(|_| format!("{name} has no digest"))?;
            let sha256 = digest
                .strip_prefix("sha256:")
                .filter(|h| is_hex64(h))
                .ok_or_else(|| format!("{name} has a digest that is not SHA-256: {digest}"))?
                .to_ascii_lowercase();
            Ok(ReleaseAsset {
                name,
                sha256,
                url: text(a, "browser_download_url")?,
            })
        })
        .collect()
}

/// `sha256sum` output: `<64 hex>  <name>` (or `<hex> *<name>`) per line.
fn parse_sums(body: &[u8]) -> Result<Vec<(String, String)>, String> {
    let text = std::str::from_utf8(body).map_err(|_| format!("{SUMS_NAME} is not text"))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let (hash, rest) = line
                .split_once(' ')
                .ok_or_else(|| format!("{SUMS_NAME} has a line without a name: {line}"))?;
            let name = rest.trim_start_matches([' ', '*']);
            if !is_hex64(hash) || name.is_empty() {
                return Err(format!(
                    "{SUMS_NAME} has a line that is not a hash and a name: {line}"
                ));
            }
            Ok((hash.to_ascii_lowercase(), name.to_owned()))
        })
        .collect()
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `scheme://host[:port]` of `url`, or `None` when it has no scheme or names a user.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase()
    ))
}

/// `major.minor.patch`, with an optional `-pre` part that sorts before the release.
fn parse_version(text: &str) -> Option<(u64, u64, u64, bool)> {
    let (core, pre) = match text.split_once('-') {
        Some((core, _)) => (core, true),
        None => (text, false),
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch, !pre))
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
