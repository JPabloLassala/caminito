//! The drawer's HTTP client (214): a GET over GIO's sockets, whose TLS is GLib's own
//! (glib-networking), the one GTK and gvfs already use, so the workspace gets no TLS stack
//! of its own and no new crate. It asks in HTTP/1.0 with `Connection: close`, so the body
//! is everything up to the end, follows a few redirects, and runs on GLib's loop.
//!
//! The pure part, reading a response, is unit-tested; nothing here is a test's way to the
//! network.

use std::fmt;

use gio::glib;
use gio::prelude::*;

use crate::daily::Fetch;

/// The most redirects followed for one request.
const REDIRECTS: usize = 5;

/// The largest response read, so a server can't fill the memory: a 4K JPEG is a few MB.
const LARGEST: usize = 64 << 20;

/// How long a connection may be silent, in seconds.
const TIMEOUT: u32 = 30;

/// Who asks, as Wikimedia's policy wants a client named, with a way to reach its authors.
const USER_AGENT: &str = concat!(
    "caminito-wallpaper/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

/// Why a request failed.
#[derive(Debug)]
pub enum Error {
    /// The URL isn't `http://` or `https://` with a host.
    Url(String),
    /// The connection, or the TLS handshake, failed: no network, most often.
    Connect(glib::Error),
    /// Sending or receiving failed.
    Io(glib::Error),
    /// What came back isn't an HTTP response.
    Malformed,
    /// The server answered with this status.
    Status(u16),
    /// More redirects than [`REDIRECTS`].
    Redirects,
    /// The response is larger than [`LARGEST`].
    TooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Url(url) => write!(f, "can't ask for {url}"),
            Error::Connect(err) => write!(f, "can't connect: {err}"),
            Error::Io(err) => write!(f, "the connection failed: {err}"),
            Error::Malformed => write!(f, "the server's answer isn't HTTP"),
            Error::Status(status) => write!(f, "the server answered {status}"),
            Error::Redirects => write!(f, "too many redirects"),
            Error::TooLarge => write!(f, "the answer is too large"),
        }
    }
}

impl std::error::Error for Error {}

/// Where a URL points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) tls: bool,
    pub(crate) host: String,
    pub(crate) port: u16,
    /// The path and query, from its `/`.
    pub(crate) path: String,
}

impl Target {
    /// The target of an absolute `url`.
    pub(crate) fn parse(url: &str) -> Result<Self, Error> {
        let bad = || Error::Url(url.to_owned());
        let (tls, rest) = if let Some(rest) = url.strip_prefix("https://") {
            (true, rest)
        } else if let Some(rest) = url.strip_prefix("http://") {
            (false, rest)
        } else {
            return Err(bad());
        };
        let (authority, path) = match rest.find(['/', '?']) {
            Some(at) if rest[at..].starts_with('?') => (&rest[..at], format!("/{}", &rest[at..])),
            Some(at) => (&rest[..at], rest[at..].to_owned()),
            None => (rest, "/".to_owned()),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => (host, port.parse().map_err(|_| bad())?),
            None => (authority, if tls { 443 } else { 80 }),
        };
        if host.is_empty() || host.contains(['@', ' ']) {
            return Err(bad());
        }
        Ok(Target {
            tls,
            host: host.to_owned(),
            port,
            path,
        })
    }

    /// The URL a redirect's `location` points to from here: absolute, from the host's root,
    /// or from the scheme.
    pub(crate) fn resolve(&self, location: &str) -> String {
        let scheme = if self.tls { "https" } else { "http" };
        if location.contains("://") {
            location.to_owned()
        } else if let Some(rest) = location.strip_prefix("//") {
            format!("{scheme}://{rest}")
        } else {
            let base = match self.path.rfind('/') {
                _ if location.starts_with('/') => String::new(),
                Some(at) => self.path[..=at].to_owned(),
                None => "/".to_owned(),
            };
            let default = if self.tls { 443 } else { 80 };
            let port = if self.port == default {
                String::new()
            } else {
                format!(":{}", self.port)
            };
            format!("{scheme}://{}{port}{base}{location}", self.host)
        }
    }

    fn request(&self) -> String {
        format!(
            "GET {} HTTP/1.0\r\nHost: {}\r\nUser-Agent: {USER_AGENT}\r\nAccept: */*\r\nConnection: close\r\n\r\n",
            self.path, self.host
        )
    }
}

/// A response, read whole.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Response {
    pub(crate) status: u16,
    /// The `Location` header, for a redirect.
    pub(crate) location: Option<String>,
    pub(crate) body: Vec<u8>,
}

impl Response {
    /// The response in `bytes`, everything the server sent. A chunked body, which a server
    /// shouldn't send to an HTTP/1.0 client, is read too.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or(Error::Malformed)?;
        let head = std::str::from_utf8(&bytes[..end]).map_err(|_| Error::Malformed)?;
        let mut lines = head.split("\r\n");
        let status = lines
            .next()
            .filter(|line| line.starts_with("HTTP/"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|status| status.parse().ok())
            .ok_or(Error::Malformed)?;
        let mut location = None;
        let mut chunked = false;
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            if name.eq_ignore_ascii_case("location") {
                location = Some(value.to_owned());
            } else if name.eq_ignore_ascii_case("transfer-encoding") {
                chunked = value.eq_ignore_ascii_case("chunked");
            }
        }
        let body = &bytes[end + 4..];
        let body = if chunked {
            unchunk(body)?
        } else {
            body.to_vec()
        };
        Ok(Response {
            status,
            location,
            body,
        })
    }
}

/// A chunked body's bytes.
fn unchunk(mut body: &[u8]) -> Result<Vec<u8>, Error> {
    let mut whole = Vec::with_capacity(body.len());
    loop {
        let line_end = body
            .windows(2)
            .position(|window| window == b"\r\n")
            .ok_or(Error::Malformed)?;
        let size = std::str::from_utf8(&body[..line_end]).map_err(|_| Error::Malformed)?;
        let size = size.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size, 16).map_err(|_| Error::Malformed)?;
        body = &body[line_end + 2..];
        if size == 0 {
            return Ok(whole);
        }
        let chunk = body.get(..size).ok_or(Error::Malformed)?;
        whole.extend_from_slice(chunk);
        body = body.get(size + 2..).ok_or(Error::Malformed)?;
    }
}

/// The client: GIO's sockets, on GLib's loop.
#[derive(Debug, Default, Clone, Copy)]
pub struct Http;

impl Fetch for Http {
    async fn get(&self, url: &str) -> Result<Vec<u8>, Error> {
        let mut url = url.to_owned();
        for _ in 0..=REDIRECTS {
            let target = Target::parse(&url)?;
            let response = Response::parse(&exchange(&target).await?)?;
            match (response.status, response.location) {
                (200..=299, _) => return Ok(response.body),
                (301 | 302 | 303 | 307 | 308, Some(location)) => url = target.resolve(&location),
                (status, _) => return Err(Error::Status(status)),
            }
        }
        Err(Error::Redirects)
    }
}

/// Sends `target`'s request, and reads everything that comes back.
async fn exchange(target: &Target) -> Result<Vec<u8>, Error> {
    let client = gio::SocketClient::new();
    client.set_tls(target.tls);
    client.set_timeout(TIMEOUT);
    // The host's name, which TLS checks the certificate against.
    let connection = client
        .connect_to_host_future(&format!("{}:{}", target.host, target.port), target.port)
        .await
        .map_err(Error::Connect)?;
    connection
        .output_stream()
        .write_all_future(target.request().into_bytes(), glib::Priority::DEFAULT)
        .await
        .map_err(|(_, err)| Error::Io(err))?;
    let input = connection.input_stream();
    let mut response = Vec::new();
    loop {
        let bytes = input
            .read_bytes_future(1 << 16, glib::Priority::DEFAULT)
            .await
            .map_err(Error::Io)?;
        if bytes.is_empty() {
            break;
        }
        response.extend_from_slice(&bytes);
        if response.len() > LARGEST {
            return Err(Error::TooLarge);
        }
    }
    // A TLS close that fails changes nothing: everything was read.
    let _ = connection.close_future(glib::Priority::DEFAULT).await;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_is_its_host_port_and_path() {
        assert_eq!(
            Target::parse("https://www.bing.com/HPImageArchive.aspx?format=js").unwrap(),
            Target {
                tls: true,
                host: "www.bing.com".into(),
                port: 443,
                path: "/HPImageArchive.aspx?format=js".into(),
            }
        );
        assert_eq!(
            Target::parse("http://localhost:8080").unwrap(),
            Target {
                tls: false,
                host: "localhost".into(),
                port: 8080,
                path: "/".into(),
            }
        );
        assert_eq!(Target::parse("https://h?q").unwrap().path, "/?q");
        for bad in [
            "ftp://host/",
            "https:///path",
            "host/path",
            "https://h:port/",
        ] {
            assert!(Target::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_redirect_is_resolved_from_the_url_it_answers() {
        let target = Target::parse("https://apod.nasa.gov/apod/astropix.html").unwrap();
        assert_eq!(target.resolve("https://x.org/a"), "https://x.org/a");
        assert_eq!(target.resolve("//cdn.org/a.jpg"), "https://cdn.org/a.jpg");
        assert_eq!(
            target.resolve("/apod/ap261015.html"),
            "https://apod.nasa.gov/apod/ap261015.html"
        );
        assert_eq!(
            target.resolve("image/a.jpg"),
            "https://apod.nasa.gov/apod/image/a.jpg"
        );
    }

    #[test]
    fn the_request_is_http_1_0_and_names_the_client() {
        let request = Target::parse("https://en.wikipedia.org/api/x")
            .unwrap()
            .request();
        assert!(request.starts_with("GET /api/x HTTP/1.0\r\nHost: en.wikipedia.org\r\n"));
        assert!(request.contains("User-Agent: caminito-wallpaper/"));
        assert!(request.ends_with("\r\n\r\n"));
    }

    #[test]
    fn a_response_is_its_status_location_and_body() {
        let response =
            Response::parse(b"HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\n\r\n\xff\xd8\xff")
                .unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"\xff\xd8\xff");

        let response =
            Response::parse(b"HTTP/1.0 302 Found\r\nlocation: /elsewhere\r\n\r\n").unwrap();
        assert_eq!(
            (response.status, response.location.as_deref()),
            (302, Some("/elsewhere"))
        );

        for bad in [
            &b"200 OK\r\n\r\n"[..],
            b"HTTP/1.1 OK\r\n\r\n",
            b"HTTP/1.1 200 OK\r\n",
        ] {
            assert!(Response::parse(bad).is_err());
        }
    }

    #[test]
    fn a_chunked_body_is_read_whole() {
        let response = Response::parse(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n6;x=y\r\npedia \r\n3\r\nin \r\n0\r\n\r\n",
        )
        .unwrap();
        assert_eq!(response.body, b"Wikipedia in ");
        assert!(
            Response::parse(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n9\r\nshort")
                .is_err()
        );
    }
}
