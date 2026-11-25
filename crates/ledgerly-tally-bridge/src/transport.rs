//! HTTP transport (card B1): one XML request in, one decoded XML response out, over Tally's
//! built-in HTTP server (default port 9000, no authentication, no TLS; spec 6.7, research R8 2.1).
//!
//! Frozen interface (plan card B1):
//!
//! ```text
//! pub struct HttpTransport { .. }
//! impl HttpTransport { pub fn new(host: &str, port: u16) -> Self; }
//! impl TallyTransport for HttpTransport { .. }
//! ```
//!
//! Nothing here touches the network until [`TallyTransport::post`] is called, which only
//! happens on an explicit user action (rule D5).
//!
//! Tally may answer in UTF-16 (the open-source loaders in research R8 send and receive
//! UTF-16LE), so the body is read as bytes and decoded here: a byte-order mark wins, then a
//! NUL byte pattern that marks BOM-less UTF-16, otherwise strict UTF-8. Text that cannot be
//! decoded is an error, never a lossy guess.

use std::io::ErrorKind;
use std::time::Duration;

use ureq::Agent;

use crate::seam::{TallyTransport, TransportError};

/// The port TallyPrime's HTTP server listens on unless the user changed it.
pub const DEFAULT_PORT: u16 = 9000;

/// Seconds a request may take before it is abandoned (connect, send and reading the reply).
pub const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Largest reply accepted, in bytes. Voucher batches are limited to 5,000 per request (spec 6.7),
/// so a reply beyond this means something is wrong rather than that Tally has a lot of data.
const MAX_RESPONSE_BYTES: u64 = 256 * 1024 * 1024;

/// Talks to a running Tally over HTTP.
#[derive(Debug, Clone)]
pub struct HttpTransport {
    agent: Agent,
    url: String,
    port: u16,
    timeout_secs: u64,
}

impl HttpTransport {
    /// A transport for `http://<host>:<port>/` with the default timeout.
    ///
    /// `host` is a host name or an IP address (an IPv6 literal may be given with or without
    /// brackets). No connection is made here; a bad host shows up as an error from
    /// [`TallyTransport::post`].
    #[must_use]
    pub fn new(host: &str, port: u16) -> Self {
        Self::with_timeout(host, port, DEFAULT_TIMEOUT_SECS)
    }

    /// Like [`HttpTransport::new`] with an explicit timeout in seconds (at least 1).
    #[must_use]
    pub fn with_timeout(host: &str, port: u16, timeout_secs: u64) -> Self {
        let timeout_secs = timeout_secs.max(1);
        let config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(timeout_secs)))
            // Tally is on this machine or the LAN: never route it through an environment proxy.
            .proxy(None)
            .build();
        let host = host.trim();
        let host = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        Self {
            agent: Agent::new_with_config(config),
            url: format!("http://{host}:{port}/"),
            port,
            timeout_secs,
        }
    }

    /// The URL requests are posted to.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The port requests are posted to.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "ureq::Error is #[non_exhaustive]: a catch-all arm is required, and every other error is reported as I/O with its text"
    )]
    fn map_error(&self, error: &ureq::Error) -> TransportError {
        match error {
            ureq::Error::StatusCode(status) => TransportError::Http { status: *status },
            ureq::Error::Timeout(_) => TransportError::Timeout {
                seconds: self.timeout_secs,
            },
            ureq::Error::Io(io) if io.kind() == ErrorKind::ConnectionRefused => {
                TransportError::TallyNotRunning { port: self.port }
            }
            ureq::Error::Io(io) if io.kind() == ErrorKind::TimedOut => TransportError::Timeout {
                seconds: self.timeout_secs,
            },
            other => TransportError::Io(other.to_string()),
        }
    }
}

impl TallyTransport for HttpTransport {
    fn post(&self, xml_request: &str) -> Result<String, TransportError> {
        tracing::debug!(
            port = self.port,
            bytes = xml_request.len(),
            "posting request to the other program"
        );
        let mut response = self
            .agent
            .post(self.url.as_str())
            .content_type("text/xml; charset=utf-8")
            .send(xml_request)
            .map_err(|e| self.map_error(&e))?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(|e| self.map_error(&e))?;
        decode_body(&bytes)
    }
}

/// Decodes a response body to text. See the module docs for the order of checks.
///
/// # Errors
/// [`TransportError::Io`] naming the byte position when the bytes are not valid in the
/// encoding they claim.
pub(crate) fn decode_body(bytes: &[u8]) -> Result<String, TransportError> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return decode_utf8(rest, 3);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, true, 2);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, false, 2);
    }
    // Without a BOM, an XML document starts with an ASCII '<'; in UTF-16 that is a NUL beside it.
    match bytes {
        [b'<', 0, ..] => decode_utf16(bytes, true, 0),
        [0, b'<', ..] => decode_utf16(bytes, false, 0),
        [..] => decode_utf8(bytes, 0),
    }
}

fn decode_utf8(bytes: &[u8], base: usize) -> Result<String, TransportError> {
    std::str::from_utf8(bytes).map(str::to_owned).map_err(|e| {
        TransportError::Io(format!(
            "reply is not valid UTF-8 at byte {}",
            base.saturating_add(e.valid_up_to())
        ))
    })
}

fn decode_utf16(bytes: &[u8], little_endian: bool, base: usize) -> Result<String, TransportError> {
    let (pairs, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return Err(TransportError::Io(format!(
            "reply is UTF-16 but has an odd number of bytes (ends at byte {})",
            base.saturating_add(bytes.len())
        )));
    }
    let units = pairs.iter().map(|&[first, second]| {
        if little_endian {
            u16::from_le_bytes([first, second])
        } else {
            u16::from_be_bytes([first, second])
        }
    });
    let mut out = String::new();
    let mut offset = base;
    for unit in char::decode_utf16(units) {
        match unit {
            Ok(c) => {
                out.push(c);
                // Two bytes per UTF-16 code unit: a surrogate pair is four bytes.
                offset = offset.saturating_add(c.len_utf16().saturating_mul(2));
            }
            Err(_) => {
                return Err(TransportError::Io(format!(
                    "reply has an unpaired UTF-16 surrogate near byte {offset}"
                )));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    // provenance: derived. Every byte sequence below is written by hand from the UTF-8 and
    // UTF-16 definitions (Unicode Standard ch. 3); no expected value is produced by our code.
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        reason = "test code: rule B2 lets tests unwrap and panic"
    )]

    use super::*;

    #[test]
    fn utf8_plain_and_with_bom() {
        assert_eq!(decode_body(b"<A/>").unwrap(), "<A/>");
        assert_eq!(
            decode_body(&[0xEF, 0xBB, 0xBF, b'<', b'A', b'/', b'>']).unwrap(),
            "<A/>"
        );
    }

    #[test]
    fn utf8_rupee_sign_is_kept() {
        // U+20B9 INDIAN RUPEE SIGN is E2 82 B9 in UTF-8.
        assert_eq!(decode_body(&[0xE2, 0x82, 0xB9]).unwrap(), "\u{20B9}");
    }

    #[test]
    fn utf16le_with_bom() {
        // FF FE, then '<' = 3C 00, 'A' = 41 00, U+20B9 = B9 20 (little endian).
        let bytes = [0xFF, 0xFE, 0x3C, 0x00, 0x41, 0x00, 0xB9, 0x20];
        assert_eq!(decode_body(&bytes).unwrap(), "<A\u{20B9}");
    }

    #[test]
    fn utf16be_with_bom() {
        let bytes = [0xFE, 0xFF, 0x00, 0x3C, 0x00, 0x41, 0x20, 0xB9];
        assert_eq!(decode_body(&bytes).unwrap(), "<A\u{20B9}");
    }

    #[test]
    fn utf16le_without_bom_is_detected_from_the_first_tag_char() {
        let bytes = [0x3C, 0x00, 0x41, 0x00, 0x2F, 0x00, 0x3E, 0x00];
        assert_eq!(decode_body(&bytes).unwrap(), "<A/>");
    }

    #[test]
    fn utf16_surrogate_pair_decodes() {
        // U+1F600 is D83D DE00; little endian bytes 3D D8 00 DE.
        let bytes = [0xFF, 0xFE, 0x3D, 0xD8, 0x00, 0xDE];
        assert_eq!(decode_body(&bytes).unwrap(), "\u{1F600}");
    }

    #[test]
    fn invalid_utf8_reports_the_byte_position() {
        // 0xFF can never start a UTF-8 sequence; it is byte 2 here.
        let err = decode_body(&[b'<', b'A', 0xFF]).unwrap_err();
        assert_eq!(
            err,
            TransportError::Io("reply is not valid UTF-8 at byte 2".to_owned())
        );
    }

    #[test]
    fn odd_length_utf16_is_an_error() {
        let err = decode_body(&[0xFF, 0xFE, 0x3C]).unwrap_err();
        assert!(matches!(err, TransportError::Io(_)));
    }

    #[test]
    fn unpaired_surrogate_is_an_error() {
        // D800 alone is a high surrogate with no low surrogate after it.
        let err = decode_body(&[0xFF, 0xFE, 0x00, 0xD8]).unwrap_err();
        assert!(matches!(err, TransportError::Io(_)));
    }

    #[test]
    fn url_is_built_from_host_and_port() {
        let t = HttpTransport::new("localhost", DEFAULT_PORT);
        assert_eq!(t.url(), "http://localhost:9000/");
        assert_eq!(t.port(), 9000);
    }

    #[test]
    fn ipv6_literal_gets_brackets_once() {
        assert_eq!(HttpTransport::new("::1", 9000).url(), "http://[::1]:9000/");
        assert_eq!(
            HttpTransport::new("[::1]", 9000).url(),
            "http://[::1]:9000/"
        );
    }
}
