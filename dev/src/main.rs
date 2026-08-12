// Dream Melody — a notation editor that stores scale degrees, not pitches.
// Copyright (C) 2026 mizaimao
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License
// for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Runs Dream Melody locally: `cargo run -p dream-melody-dev`.
//!
//! The browser will not load a wasm module from a `file://` URL, so something
//! has to serve the directory. This is that something, and it opens the app
//! once it is listening.
//!
//! It binds to the loopback address only and has no dependencies — the
//! standard library is enough to hand files to a browser on the same machine,
//! and a development runner should never be a reason to audit a supply chain.
//!
//! This is scaffolding. The real application is the desktop shell in stage 10,
//! which owns its own window and needs no browser at all.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::thread;

/// The largest request line and header block worth reading from a local
/// browser. Anything longer is not a request this needs to answer.
const MAX_REQUEST_BYTES: usize = 16 * 1024;

fn main() -> std::io::Result<()> {
    let root = web_root();

    if !root.join("index.html").is_file() {
        eprintln!("cannot find the app at {}", root.display());
        eprintln!("run this from inside the repository");
        std::process::exit(1);
    }

    // The wasm is generated, not committed, so a fresh clone has none. Say so
    // plainly rather than serving a page that fails in the console.
    if !root.join("wasm/dream_melody_wasm.js").is_file() {
        eprintln!("the wasm module has not been built yet");
        eprintln!("run core/wasm/build.sh first, then try again");
        std::process::exit(1);
    }

    // Port zero asks the operating system for a free one, so two copies of
    // this never fight over a number.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}/");

    println!("Dream Melody is running at {url}");
    println!("serving {}", root.display());
    println!("press control-C to stop");
    open_in_browser(&url);

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let root = root.clone();
        // A browser opens several connections at once for one page. Give each
        // its own thread rather than making the page load in series.
        thread::spawn(move || {
            if let Err(error) = serve(stream, &root) {
                eprintln!("dropped a connection: {error}");
            }
        });
    }
    Ok(())
}

/// The `web` directory, found relative to this crate rather than to wherever
/// the command happened to be run from.
fn web_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."))
        .join("web")
}

/// Answers one request.
fn serve(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);

    let mut request_line = String::new();
    reader
        .by_ref()
        .take(MAX_REQUEST_BYTES as u64)
        .read_line(&mut request_line)?;

    // Headers are read and discarded: nothing here varies by them.
    let mut header = String::new();
    loop {
        header.clear();
        let read = reader
            .by_ref()
            .take(MAX_REQUEST_BYTES as u64)
            .read_line(&mut header)?;
        if read == 0 || header.trim().is_empty() {
            break;
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");

    if method != "GET" && method != "HEAD" {
        return respond(&mut stream, 405, "text/plain", b"only GET is served");
    }

    let Some(path) = resolve(root, target) else {
        return respond(&mut stream, 403, "text/plain", b"outside the app");
    };

    match fs::read(&path) {
        Ok(body) => {
            let kind = content_type(&path);
            if method == "HEAD" {
                return respond(&mut stream, 200, kind, b"");
            }
            respond(&mut stream, 200, kind, &body)
        }
        Err(_) => respond(&mut stream, 404, "text/plain", b"no such file"),
    }
}

/// Undoes the percent-escaping a browser applies to a URL.
///
/// A stray `%` or a bad pair is left as written rather than guessed at.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let pair = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
            if let Some(byte) = pair.and_then(|pair| u8::from_str_radix(pair, 16).ok()) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Turns a request target into a path inside the app, or nothing.
///
/// Every `..` is rejected rather than resolved, and the check happens *after*
/// unescaping, so a `%2e%2e` is caught like a plain one. This listens on
/// loopback only, but "only I can reach it" is a poor reason to hand out the
/// filesystem.
fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let path = percent_decode(path);
    let path = if path == "/" { "/index.html" } else { &path };

    let mut resolved = root.to_path_buf();
    for component in Path::new(path.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            // Anything that is not a plain name is an attempt to leave.
            _ => return None,
        }
    }
    Some(resolved)
}

/// What to call a file, by extension.
///
/// A browser refuses to instantiate a wasm module served as anything but
/// `application/wasm`, and refuses a module script that is not JavaScript, so
/// these two matter more than the rest.
fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("svg") => "image/svg+xml",
        Some("mid") | Some("midi") => "audio/midi",
        Some("xml") | Some("musicxml") => "application/xml",
        Some("txt") | Some("abc") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn respond(stream: &mut TcpStream, status: u16, kind: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Method Not Allowed",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {kind}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()
}

/// Opens the app in whatever the machine considers its browser.
///
/// Failing to open is not failing to run: the URL is printed either way.
fn open_in_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        Some(("open", vec![url.to_string()]))
    } else if cfg!(target_os = "windows") {
        Some((
            "cmd",
            vec!["/C".into(), "start".into(), String::new(), url.into()],
        ))
    } else {
        Some(("xdg-open", vec![url.to_string()]))
    };

    if let Some((command, arguments)) = opener
        && Command::new(command).args(arguments).spawn().is_err()
    {
        println!("(could not open a browser; visit the address above)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_is_the_page() {
        let root = Path::new("/app");
        assert_eq!(resolve(root, "/"), Some(PathBuf::from("/app/index.html")));
        assert_eq!(
            resolve(root, "/?cache=1"),
            Some(PathBuf::from("/app/index.html"))
        );
    }

    #[test]
    fn ordinary_paths_resolve_inside_the_app() {
        let root = Path::new("/app");
        assert_eq!(
            resolve(root, "/wasm/dream_melody_wasm_bg.wasm"),
            Some(PathBuf::from("/app/wasm/dream_melody_wasm_bg.wasm"))
        );
        assert_eq!(
            resolve(root, "/fixture-a.json?v=2#top"),
            Some(PathBuf::from("/app/fixture-a.json"))
        );
    }

    #[test]
    fn nothing_escapes_the_app_directory() {
        let root = Path::new("/app");
        for target in [
            "/../secrets",
            "/wasm/../../secrets",
            "/./../../etc/passwd",
            "//../secrets",
            // Escaped, which is the version that gets past a naive check made
            // before unescaping.
            "/%2e%2e/secrets",
            "/wasm/%2E%2E/%2E%2E/secrets",
        ] {
            assert_eq!(resolve(root, target), None, "{target} escaped");
        }
    }

    #[test]
    fn escaped_names_are_read_as_names() {
        let root = Path::new("/app");
        assert_eq!(
            resolve(root, "/a%20name.json"),
            Some(PathBuf::from("/app/a name.json"))
        );
        // A lone percent is a filename, not an error.
        assert_eq!(
            resolve(root, "/100%.json"),
            Some(PathBuf::from("/app/100%.json"))
        );
    }

    #[test]
    fn the_two_types_a_browser_insists_on_are_right() {
        assert_eq!(content_type(Path::new("a.wasm")), "application/wasm");
        assert_eq!(
            content_type(Path::new("a.js")),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type(Path::new("a.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(content_type(Path::new("a.bin")), "application/octet-stream");
    }
}
