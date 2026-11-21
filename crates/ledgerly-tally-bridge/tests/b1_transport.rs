//! Card B1 tests: `HttpTransport` against a throw-away server on the loopback interface.
//!
//! Provenance. The HTTP behaviour asserted (method, path, status handling, content length)
//! is `derived` from RFC 9110/9112; the bodies are fictional literals written here. Nothing
//! leaves the machine: the server binds 127.0.0.1 on an ephemeral port. No expected value is
//! copied from our own output.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ledgerly_tally_bridge::discover::list_companies;
use ledgerly_tally_bridge::transport::{DEFAULT_PORT, HttpTransport};
use ledgerly_tally_bridge::{TallyTransport, TransportError};

fn read_request(stream: &mut TcpStream) -> String {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0, "client closed before sending a full request");
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let length: usize = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .map_or(0, |v| v.trim().parse().unwrap());
    while buf.len() < header_end + length {
        let n = stream.read(&mut chunk).unwrap();
        assert!(n > 0, "client closed before sending the whole body");
        buf.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn http_response(status_line: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

/// Serves one connection: reads the request, writes `response`, returns the request text.
fn serve_once(response: Vec<u8>) -> (u16, JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_request(&mut stream);
        stream.write_all(&response).unwrap();
        stream.flush().unwrap();
        request
    });
    (port, handle)
}

#[test]
fn default_port_is_9000() {
    // Spec 6.7 and research R8 2.1: TallyPrime's HTTP server defaults to port 9000.
    assert_eq!(DEFAULT_PORT, 9000);
}

#[test]
fn posts_the_xml_and_returns_the_body() {
    let (port, server) = serve_once(http_response("200 OK", b"<ENVELOPE>ok</ENVELOPE>"));
    let t = HttpTransport::new("127.0.0.1", port);
    let reply = t.post("<ENVELOPE>ask</ENVELOPE>").unwrap();
    assert_eq!(reply, "<ENVELOPE>ok</ENVELOPE>");
    let request = server.join().unwrap();
    assert!(request.starts_with("POST / HTTP/1.1\r\n"));
    assert!(request.ends_with("<ENVELOPE>ask</ENVELOPE>"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("content-type: text/xml")
    );
}

#[test]
fn utf16le_reply_is_decoded() {
    // "<A>\u{20B9}</A>" as UTF-16LE with a byte-order mark, written out by hand:
    // FF FE | 3C 00 | 41 00 | 3E 00 | B9 20 | 3C 00 | 2F 00 | 41 00 | 3E 00.
    let body = [
        0xFF, 0xFE, 0x3C, 0x00, 0x41, 0x00, 0x3E, 0x00, 0xB9, 0x20, 0x3C, 0x00, 0x2F, 0x00, 0x41,
        0x00, 0x3E, 0x00,
    ];
    let (port, server) = serve_once(http_response("200 OK", &body));
    let t = HttpTransport::new("127.0.0.1", port);
    assert_eq!(t.post("<X/>").unwrap(), "<A>\u{20B9}</A>");
    server.join().unwrap();
}

#[test]
fn http_error_status_is_reported() {
    let (port, server) = serve_once(http_response("500 Internal Server Error", b"oops"));
    let t = HttpTransport::new("127.0.0.1", port);
    assert_eq!(
        t.post("<X/>").unwrap_err(),
        TransportError::Http { status: 500 }
    );
    server.join().unwrap();
}

#[test]
fn nothing_listening_is_tally_not_running_with_the_port() {
    // Bind to get a free port, then close the listener so nothing is listening on it.
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let t = HttpTransport::new("127.0.0.1", port);
    assert_eq!(
        t.post("<X/>").unwrap_err(),
        TransportError::TallyNotRunning { port }
    );
}

#[test]
fn silent_server_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = read_request(&mut stream);
        // Never answer: hold the connection longer than the client's one-second timeout.
        thread::sleep(Duration::from_secs(3));
    });
    let t = HttpTransport::with_timeout("127.0.0.1", port, 1);
    assert_eq!(
        t.post("<X/>").unwrap_err(),
        TransportError::Timeout { seconds: 1 }
    );
    server.join().unwrap();
}

#[test]
fn list_companies_over_http() {
    let reply = "<ENVELOPE><BODY><DATA><COLLECTION>\
        <COMPANY NAME=\"Acme Traders\"/></COLLECTION></DATA></BODY></ENVELOPE>";
    let (port, server) = serve_once(http_response("200 OK", reply.as_bytes()));
    let t = HttpTransport::new("127.0.0.1", port);
    assert_eq!(list_companies(&t).unwrap(), vec!["Acme Traders".to_owned()]);
    let request = server.join().unwrap();
    assert!(request.contains("<TYPE>Company</TYPE>"));
}
