//! The sign-in's loopback listener: the only socket the host opens that a peer connects to
//! (`specs/005-plugin-credentials/research.md`, R2). It listens on `127.0.0.1` at a port the
//! system chooses, for one sign-in, and closes after the first request to `/callback` or at the
//! sign-in's time limit, whichever comes first.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

/// The path the service redirects the browser to.
const PATH: &str = "/callback";
/// How often the listener looks for a connection while it waits.
const POLL: Duration = Duration::from_millis(20);

/// The page the browser shows once the sign-in reached the host: fixed text, never anything from
/// the request, and never kept in the browser's cache.
const PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>Dashboard sign-in</title>\
<p>The sign-in reached the dashboard. You can close this window and go back to it.</p>";

/// A listener bound for one sign-in.
pub(crate) struct Callback {
    listener: TcpListener,
    port: u16,
}

impl Callback {
    /// Binds `127.0.0.1` at a port the system chooses.
    pub(crate) fn bind() -> io::Result<Callback> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        Ok(Callback { listener, port })
    }

    /// The redirect address the service sends the browser to (RFC 8252, §7.3).
    pub(crate) fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}{PATH}", self.port)
    }

    /// Waits for the first `GET /callback` until `deadline`, and returns its query, or `None` at
    /// the deadline. Any other request gets `404` and does not count; a head over `max_head`
    /// bytes, or slower than `read_time`, is dropped. The listener closes when this returns.
    pub(crate) fn wait(
        self,
        deadline: Instant,
        max_head: usize,
        read_time: Duration,
    ) -> io::Result<Option<Zeroizing<String>>> {
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(query) = serve(stream, max_head, read_time) {
                        return Ok(Some(query));
                    }
                }
                // Nothing to accept yet, or a connection its peer already gave up on
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                            | io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::ConnectionReset
                    ) => {}
                Err(error) => return Err(error),
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            thread::sleep(POLL.min(deadline - now));
        }
    }
}

/// Reads one request head and answers it: the query of a `GET /callback`, or `None` for anything
/// else, which gets `404` or nothing at all.
fn serve(mut stream: TcpStream, max_head: usize, read_time: Duration) -> Option<Zeroizing<String>> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(read_time)).ok()?;
    stream.set_write_timeout(Some(read_time)).ok()?;
    let started = Instant::now();
    let mut head = Zeroizing::new(Vec::with_capacity(1024));
    let mut chunk = [0u8; 1024];
    loop {
        // The size first: a head that ends past the limit is as oversized as one that does not end
        if head.len() > max_head || started.elapsed() > read_time {
            return None;
        }
        if head.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(read) => head.extend_from_slice(&chunk[..read]),
        }
    }
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut request = httparse::Request::new(&mut headers);
    let parsed = request.parse(&head).ok()?;
    if parsed.is_partial() {
        return None;
    }
    let target = request.path.unwrap_or_default();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if request.method == Some("GET") && path == PATH {
        let _ = answer(&mut stream, "200 OK", PAGE);
        return Some(Zeroizing::new(query.to_owned()));
    }
    let _ = answer(&mut stream, "404 Not Found", "");
    None
}

fn answer(stream: &mut TcpStream, status: &str, body: &str) -> io::Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\n\
Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: usize = 8 * 1024;
    const READ: Duration = Duration::from_millis(500);

    /// Sends `request` to `port` and returns the response, if one comes.
    fn send(port: u16, request: &[u8]) -> String {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream.write_all(request).unwrap();
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        response
    }

    /// Runs `wait` on a thread, for the tests to play the browser.
    fn waiting(callback: Callback, within: Duration) -> thread::JoinHandle<Option<String>> {
        thread::spawn(move || {
            callback
                .wait(Instant::now() + within, LIMIT, READ)
                .unwrap()
                .map(|query| query.to_string())
        })
    }

    #[test]
    fn it_listens_on_the_loopback_address_at_a_port_the_system_chose() {
        let callback = Callback::bind().unwrap();
        assert_ne!(callback.port, 0);
        assert_eq!(
            callback.redirect_uri(),
            format!("http://127.0.0.1:{}/callback", callback.port)
        );
        assert!(callback.listener.local_addr().unwrap().ip().is_loopback());
    }

    #[test]
    fn the_first_callback_ends_the_wait_and_gets_a_fixed_page() {
        let callback = Callback::bind().unwrap();
        let port = callback.port;
        let waiter = waiting(callback, Duration::from_secs(10));
        // Another path does not count
        let other = send(port, b"GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n");
        assert!(other.starts_with("HTTP/1.1 404"), "{other}");
        let page = send(
            port,
            b"GET /callback?code=c0de&state=<script>x</script> HTTP/1.1\r\nHost: x\r\n\r\n",
        );
        assert!(page.starts_with("HTTP/1.1 200"), "{page}");
        assert!(page.contains("Cache-Control: no-store"), "{page}");
        assert!(!page.contains("c0de") && !page.contains("<script>"), "{page}");
        assert_eq!(
            waiter.join().unwrap().as_deref(),
            Some("code=c0de&state=<script>x</script>")
        );
        // Closed once it returned
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());
    }

    #[test]
    fn a_post_or_a_path_that_only_starts_like_the_callback_does_not_count() {
        let callback = Callback::bind().unwrap();
        let port = callback.port;
        let waiter = waiting(callback, Duration::from_secs(10));
        for request in [
            &b"POST /callback?code=x HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\n\r\n"[..],
            b"GET /callbackx?code=x HTTP/1.1\r\nHost: x\r\n\r\n",
            b"GET /callback/x HTTP/1.1\r\nHost: x\r\n\r\n",
        ] {
            assert!(send(port, request).starts_with("HTTP/1.1 404"));
        }
        send(port, b"GET /callback?code=last HTTP/1.1\r\nHost: x\r\n\r\n");
        assert_eq!(waiter.join().unwrap().as_deref(), Some("code=last"));
    }

    #[test]
    fn an_oversized_or_slow_head_is_dropped_and_the_wait_goes_on() {
        let callback = Callback::bind().unwrap();
        let port = callback.port;
        let waiter = waiting(callback, Duration::from_secs(10));
        let mut huge = b"GET /callback?code=".to_vec();
        huge.extend(std::iter::repeat_n(b'a', LIMIT + 10));
        huge.extend_from_slice(b" HTTP/1.1\r\n\r\n");
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        let _ = stream.write_all(&huge);
        drop(stream);
        // A connection that sends nothing until the read time runs out
        let silent = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        thread::sleep(READ + Duration::from_millis(200));
        drop(silent);
        send(port, b"GET /callback?code=ok HTTP/1.1\r\nHost: x\r\n\r\n");
        assert_eq!(waiter.join().unwrap().as_deref(), Some("code=ok"));
    }

    #[test]
    fn the_wait_ends_at_its_deadline_and_closes_the_listener() {
        let callback = Callback::bind().unwrap();
        let port = callback.port;
        let started = Instant::now();
        let query = callback
            .wait(Instant::now() + Duration::from_millis(200), LIMIT, READ)
            .unwrap();
        assert!(query.is_none());
        assert!(started.elapsed() >= Duration::from_millis(200));
        assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());
    }
}
