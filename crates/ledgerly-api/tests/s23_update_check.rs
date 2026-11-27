//! S23: the opt-in update check, against a local HTTP server that plays the GitHub releases API.
//!
//! Provenance (`derived`):
//! - The response shape is the GitHub REST "Get the latest release" response, as returned for
//!   this project on 2026-11-27 (`tag_name`, `html_url`, and `assets[]` with `name`, `digest`
//!   written `sha256:<hex>`, and `browser_download_url`). `SHA256SUMS.txt` is the `sha256sum`
//!   output the release job writes (`<64 hex>  <name>` per line).
//! - The expected hashes are computed here from the bytes the server serves (SHA-256, FIPS 180-4).
//!   The hash of "abc" is the FIPS 180-2 example value.
//! - The outcome rules come from the round s23 brief: off unless asked, only the releases API and
//!   GitHub's download hosts, the checksums must agree, and never an install.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use ledgerly_api::update_check::{UpdateCheck, UpdateChecker, sha256_of_file};
use sha2::{Digest, Sha256};

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// One canned reply: status, extra headers and body.
#[derive(Clone)]
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

fn ok(body: impl Into<Vec<u8>>) -> Reply {
    Reply {
        status: 200,
        headers: Vec::new(),
        body: body.into(),
    }
}

fn status(code: u16) -> Reply {
    Reply {
        status: code,
        headers: Vec::new(),
        body: b"{\"message\":\"no\"}".to_vec(),
    }
}

fn redirect(to: &str) -> Reply {
    Reply {
        status: 302,
        headers: vec![("Location".to_owned(), to.to_owned())],
        body: Vec::new(),
    }
}

/// A request as the server saw it: the path and the header lines, lower-cased names.
#[derive(Debug, Clone)]
struct Seen {
    path: String,
    headers: Vec<(String, String)>,
}

struct Server {
    origin: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

/// Serves `routes` (path to reply) until the test ends.
fn serve(routes: Vec<(&str, Reply)>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let routes: Vec<(String, Reply)> = routes.into_iter().map(|(p, r)| (p.to_owned(), r)).collect();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            handle(stream, &routes, &log);
        }
    });
    Server { origin, seen }
}

fn handle(mut stream: TcpStream, routes: &[(String, Reply)], log: &Mutex<Vec<Seen>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    if reader.read_line(&mut first).unwrap_or(0) == 0 {
        return;
    }
    let path = first.split_whitespace().nth(1).unwrap_or("").to_owned();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some((k, v)) = line.trim_end().split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_owned()));
        }
    }
    log.lock().unwrap().push(Seen {
        path: path.clone(),
        headers,
    });
    let reply = routes
        .iter()
        .find(|(p, _)| *p == path)
        .map_or_else(|| status(404), |(_, r)| r.clone());
    let mut head = format!(
        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.body.len()
    );
    for (k, v) in &reply.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&reply.body);
    let _ = stream.flush();
    let mut rest = Vec::new();
    let _ = reader.read_to_end(&mut rest);
}

const INSTALLER: &[u8] = b"pretend installer bytes";
const TARBALL: &[u8] = b"pretend tarball bytes";

fn sums_text(installer_hash: &str) -> String {
    format!(
        "{installer_hash}  ledgerly-1.1.0-windows-x86_64-setup.exe\n{}  ledgerly-1.1.0-linux-x86_64.tar.gz\n",
        hex(TARBALL)
    )
}

fn release_json(origin: &str, tag: &str, sums: &str, sums_digest: &str) -> String {
    let _ = sums;
    format!(
        r#"{{"tag_name":"{tag}","html_url":"{origin}/releases/tag/{tag}","assets":[
 {{"name":"ledgerly-1.1.0-windows-x86_64-setup.exe","digest":"sha256:{i}","browser_download_url":"{origin}/download/setup.exe"}},
 {{"name":"ledgerly-1.1.0-linux-x86_64.tar.gz","digest":"sha256:{t}","browser_download_url":"{origin}/download/tar.gz"}},
 {{"name":"SHA256SUMS.txt","digest":"sha256:{sums_digest}","browser_download_url":"{origin}/download/SHA256SUMS.txt"}}]}}"#,
        i = hex(INSTALLER),
        t = hex(TARBALL),
    )
}

/// A server whose latest release is `tag`, with the given sums file and the digest the API gives
/// for it. The sums download redirects once to `/assets/sums`, as GitHub redirects to its asset host.
fn release_server(tag: &str, sums: String, sums_digest: Option<String>) -> Server {
    // The origin is only known once bound, so bind first and fill the routes after.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let digest = sums_digest.unwrap_or_else(|| hex(sums.as_bytes()));
    let routes: Vec<(String, Reply)> = vec![
        (
            "/repos/x/releases/latest".to_owned(),
            ok(release_json(&origin, tag, &sums, &digest)),
        ),
        (
            "/download/SHA256SUMS.txt".to_owned(),
            redirect(&format!("{origin}/assets/sums")),
        ),
        ("/assets/sums".to_owned(), ok(sums.into_bytes())),
    ];
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            handle(stream, &routes, &log);
        }
    });
    Server { origin, seen }
}

fn checker(server: &Server) -> UpdateChecker {
    UpdateChecker::with_endpoints(
        &format!("{}/repos/x/releases/latest", server.origin),
        &[server.origin.as_str()],
    )
}

#[test]
fn a_newer_release_whose_checksums_agree_is_offered_with_its_hashes() {
    let server = release_server("v1.1.0", sums_text(&hex(INSTALLER)), None);
    let UpdateCheck::Available(release) = checker(&server).check("1.0.0") else {
        panic!("not offered");
    };
    assert_eq!(release.tag, "v1.1.0");
    assert_eq!(release.version, "1.1.0");
    assert_eq!(
        release.page_url,
        format!("{}/releases/tag/v1.1.0", server.origin)
    );
    let installer = release
        .assets
        .iter()
        .find(|a| a.name.ends_with("-setup.exe"))
        .unwrap();
    assert_eq!(installer.sha256, hex(INSTALLER));
    assert_eq!(
        installer.url,
        format!("{}/download/setup.exe", server.origin)
    );
    // The sums file itself is not listed as something to download.
    assert_eq!(release.assets.len(), 2);
    // Nothing was downloaded but the API reply and the sums file: never the installer.
    let paths: Vec<String> = server
        .seen
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.path.clone())
        .collect();
    assert_eq!(
        paths,
        vec![
            "/repos/x/releases/latest",
            "/download/SHA256SUMS.txt",
            "/assets/sums"
        ]
    );
}

#[test]
fn the_same_or_an_older_version_is_up_to_date() {
    let server = release_server("v1.0.0", sums_text(&hex(INSTALLER)), None);
    assert_eq!(
        checker(&server).check("1.0.0"),
        UpdateCheck::UpToDate {
            latest: "1.0.0".to_owned()
        }
    );
    let server = release_server("v0.9.0", sums_text(&hex(INSTALLER)), None);
    assert_eq!(
        checker(&server).check("1.0.0"),
        UpdateCheck::UpToDate {
            latest: "0.9.0".to_owned()
        }
    );
}

#[test]
fn a_sums_line_that_disagrees_with_the_asset_digest_is_never_offered() {
    let server = release_server("v1.1.0", sums_text(&hex(b"other bytes")), None);
    let UpdateCheck::ChecksumsDisagree { reason } = checker(&server).check("1.0.0") else {
        panic!("offered");
    };
    assert!(
        reason.contains("ledgerly-1.1.0-windows-x86_64-setup.exe"),
        "{reason}"
    );
}

#[test]
fn a_sums_file_that_does_not_match_its_own_digest_is_never_offered() {
    let server = release_server(
        "v1.1.0",
        sums_text(&hex(INSTALLER)),
        Some(hex(b"something else")),
    );
    let UpdateCheck::ChecksumsDisagree { reason } = checker(&server).check("1.0.0") else {
        panic!("offered");
    };
    assert!(reason.contains("SHA256SUMS.txt"), "{reason}");
}

#[test]
fn not_found_forbidden_and_rate_limited_could_not_check() {
    for code in [404, 403, 429, 500] {
        let server = serve(vec![("/repos/x/releases/latest", status(code))]);
        let UpdateCheck::CouldNotCheck { reason } = checker(&server).check("1.0.0") else {
            panic!("{code} was not a failed check");
        };
        assert!(reason.contains(&code.to_string()), "{code}: {reason}");
    }
}

#[test]
fn no_server_could_not_check() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let checker =
        UpdateChecker::with_endpoints(&format!("{origin}/repos/x/releases/latest"), &[&origin]);
    assert!(matches!(
        checker.check("1.0.0"),
        UpdateCheck::CouldNotCheck { .. }
    ));
}

#[test]
fn a_redirect_to_another_host_is_not_followed() {
    let elsewhere = serve(vec![("/sums", ok(sums_text(&hex(INSTALLER))))]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let sums = sums_text(&hex(INSTALLER));
    let routes: Vec<(String, Reply)> = vec![
        (
            "/repos/x/releases/latest".to_owned(),
            ok(release_json(
                &origin,
                "v1.1.0",
                &sums,
                &hex(sums.as_bytes()),
            )),
        ),
        (
            "/download/SHA256SUMS.txt".to_owned(),
            redirect(&format!("{}/sums", elsewhere.origin)),
        ),
    ];
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = Arc::clone(&log);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            handle(stream, &routes, &log2);
        }
    });
    let checker =
        UpdateChecker::with_endpoints(&format!("{origin}/repos/x/releases/latest"), &[&origin]);
    let UpdateCheck::CouldNotCheck { reason } = checker.check("1.0.0") else {
        panic!("followed");
    };
    assert!(reason.contains(&elsewhere.origin), "{reason}");
    assert!(elsewhere.seen.lock().unwrap().is_empty());
}

#[test]
fn a_release_without_a_sums_file_could_not_check() {
    let listener_json = |origin: &str| {
        format!(
            r#"{{"tag_name":"v1.1.0","html_url":"{origin}/r","assets":[{{"name":"ledgerly-1.1.0-linux-x86_64.tar.gz","digest":"sha256:{}","browser_download_url":"{origin}/d"}}]}}"#,
            hex(TARBALL)
        )
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let routes = vec![(
        "/repos/x/releases/latest".to_owned(),
        ok(listener_json(&origin)),
    )];
    let log = Arc::new(Mutex::new(Vec::new()));
    let log2 = Arc::clone(&log);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            handle(stream, &routes, &log2);
        }
    });
    let checker =
        UpdateChecker::with_endpoints(&format!("{origin}/repos/x/releases/latest"), &[&origin]);
    let UpdateCheck::CouldNotCheck { reason } = checker.check("1.0.0") else {
        panic!("offered");
    };
    assert!(reason.contains("SHA256SUMS.txt"), "{reason}");
}

#[test]
fn requests_carry_a_generic_user_agent_and_nothing_personal() {
    let server = release_server("v1.0.0", sums_text(&hex(INSTALLER)), None);
    let _ = checker(&server).check("1.0.0");
    let seen = server.seen.lock().unwrap().clone();
    assert!(!seen.is_empty());
    for request in &seen {
        let agent = request
            .headers
            .iter()
            .find(|(k, _)| k == "user-agent")
            .map(|(_, v)| v.clone());
        assert_eq!(
            agent.as_deref(),
            Some("Ledgerly-update-check"),
            "{request:?}"
        );
        for (k, _) in &request.headers {
            assert!(
                !["cookie", "authorization", "from"].contains(&k.as_str()),
                "{request:?}"
            );
        }
        assert!(!request.path.contains('?'), "{request:?}");
    }
}

#[test]
fn the_hash_of_a_file_is_its_sha256_in_lower_case_hex() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("abc.bin");
    std::fs::write(&path, b"abc").unwrap();
    assert_eq!(
        sha256_of_file(&path).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn the_default_checker_names_only_the_project_release_api() {
    assert_eq!(
        ledgerly_api::update_check::RELEASES_LATEST_URL,
        "https://api.github.com/repos/DevanshChoubey/tally-oss/releases/latest"
    );
}
