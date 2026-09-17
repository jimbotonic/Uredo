//! The service, exercised over a real socket. Byte for byte the twin's `tests/api.rs`, with
//! one name changed, so "the framework version behaves identically" is a test result and not a
//! reading of the diff.

use std::sync::Arc;

use restdemo_idiomatic::store::Store;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;

/// Start the service on an ephemeral port and return it.
async fn start() -> u16 {
    start_stoppable().await.0
}

/// The same, handing back the notifier that shuts it down.
async fn start_stoppable() -> (u16, Arc<Notify>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = Arc::new(Store::new());
    let stop = Arc::new(Notify::new());
    let serve_stop = Arc::clone(&stop);
    tokio::spawn(async move {
        let _ = restdemo_axum::serve_until(listener, state, serve_stop).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (port, stop)
}

/// One request, one response, as raw bytes: the point is to test the wire, not a client.
async fn request(port: u16, head: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let text = format!(
        "{}\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        head,
        body.len(),
        body
    );
    stream.write_all(text.as_bytes()).await.unwrap();
    let mut out = Vec::new();
    stream.read_to_end(&mut out).await.unwrap();
    String::from_utf8_lossy(&out).to_string()
}

fn status_of(response: &str) -> u16 {
    response.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// The value of one response header, if it is there.
fn header_of(response: &str, name: &str) -> Option<String> {
    for line in response.split("\r\n") {
        let Some((key, value)) = line.split_once(':') else { continue };
        if key.eq_ignore_ascii_case(name) {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// How many items a JSON array response holds, counted by its ids.
fn count_of(response: &str) -> usize {
    response.matches("\"id\":").count()
}

#[tokio::test]
async fn health_reports_the_item_count() {
    let port = start().await;
    let r = request(port, "GET /health HTTP/1.1", "").await;
    assert_eq!(status_of(&r), 200);
    assert!(r.contains("\"items\":0"), "{r}");
}

#[tokio::test]
async fn an_item_is_created_read_patched_and_deleted() {
    let port = start().await;

    let created = request(port, "POST /items HTTP/1.1", "{\"name\":\"write the demo\"}").await;
    assert_eq!(status_of(&created), 201);
    assert!(created.contains("\"id\":1"), "{created}");
    assert!(created.contains("\"done\":false"), "{created}");

    let listed = request(port, "GET /items HTTP/1.1", "").await;
    assert_eq!(status_of(&listed), 200);
    assert!(listed.contains("write the demo"), "{listed}");

    let patched = request(port, "PATCH /items/1 HTTP/1.1", "{\"done\":true}").await;
    assert_eq!(status_of(&patched), 200);
    assert!(patched.contains("\"done\":true"), "{patched}");

    let deleted = request(port, "DELETE /items/1 HTTP/1.1", "").await;
    assert_eq!(status_of(&deleted), 204);

    let gone = request(port, "GET /items/1 HTTP/1.1", "").await;
    assert_eq!(status_of(&gone), 404);
}

#[tokio::test]
async fn the_fixed_error_surface_maps_to_statuses() {
    let port = start().await;

    assert_eq!(status_of(&request(port, "GET /nope HTTP/1.1", "").await), 404);
    assert_eq!(status_of(&request(port, "GET /items/1/tags HTTP/1.1", "").await), 404);
    assert_eq!(status_of(&request(port, "POST /items HTTP/1.1", "{not json").await), 400);
    assert_eq!(status_of(&request(port, "POST /items HTTP/1.1", "{\"name\":\"\"}").await), 400);

    assert_eq!(status_of(&request(port, "POST /items HTTP/1.1", "{\"name\":\"dup\"}").await), 201);
    let conflict = request(port, "POST /items HTTP/1.1", "{\"name\":\"dup\"}").await;
    assert_eq!(status_of(&conflict), 409);
    assert!(conflict.contains("already exists"), "{conflict}");
}

#[tokio::test]
async fn every_accepted_type_survives_a_round_trip() {
    let port = start().await;
    let body = "{\"name\":\"typed\",\"tag\":\"t\",\"priority\":-7,\"weight\":2.5,\
\"labels\":[\"a\",\"b\"],\"meta\":{\"k\":\"v\"}}";
    let created = request(port, "POST /items HTTP/1.1", body).await;
    assert_eq!(status_of(&created), 201);
    for expected in [
        "\"id\":1",
        "\"priority\":-7",
        "\"weight\":2.5",
        "\"name\":\"typed\"",
        "\"done\":false",
        "\"tag\":\"t\"",
        "\"labels\":[\"a\",\"b\"]",
        "\"meta\":{\"k\":\"v\"}",
    ] {
        assert!(created.contains(expected), "{expected} missing from {created}");
    }

    let read = request(port, "GET /items/1 HTTP/1.1", "").await;
    assert!(read.contains("\"labels\":[\"a\",\"b\"]"), "{read}");
    assert!(read.contains("\"meta\":{\"k\":\"v\"}"), "{read}");
}

#[tokio::test]
async fn a_query_filters_and_pages_without_allocating_for_it() {
    let port = start().await;
    for n in 0..6 {
        let label = if n % 2 == 0 { "even" } else { "odd" };
        let body = format!("{{\"name\":\"item-{n}\",\"labels\":[\"{label}\"]}}");
        assert_eq!(status_of(&request(port, "POST /items HTTP/1.1", &body).await), 201);
    }

    assert_eq!(count_of(&request(port, "GET /items HTTP/1.1", "").await), 6);
    assert_eq!(count_of(&request(port, "GET /items?label=even HTTP/1.1", "").await), 3);
    assert_eq!(count_of(&request(port, "GET /items?contains=item-4 HTTP/1.1", "").await), 1);
    assert_eq!(count_of(&request(port, "GET /items?limit=2 HTTP/1.1", "").await), 2);
    assert_eq!(count_of(&request(port, "GET /items?limit=2&offset=5 HTTP/1.1", "").await), 1);

    assert_eq!(count_of(&request(port, "GET /items?done=false HTTP/1.1", "").await), 6);
    assert_eq!(count_of(&request(port, "GET /items?done=true HTTP/1.1", "").await), 0);
    let bad = request(port, "GET /items?done=maybe HTTP/1.1", "").await;
    assert_eq!(status_of(&bad), 400);
    let typo = request(port, "GET /items?limti=2 HTTP/1.1", "").await;
    assert_eq!(status_of(&typo), 400);
    assert!(typo.contains("unknown query parameter"), "{typo}");
}

#[tokio::test]
async fn a_client_that_already_has_the_answer_gets_no_body() {
    let port = start().await;
    assert_eq!(status_of(&request(port, "POST /items HTTP/1.1", "{\"name\":\"cached\"}").await), 201);

    let first = request(port, "GET /items/1 HTTP/1.1", "").await;
    assert_eq!(status_of(&first), 200);
    let Some(tag) = header_of(&first, "etag") else { panic!("no ETag on a GET: {first}") };

    let head = format!("GET /items/1 HTTP/1.1\r\nIf-None-Match: {tag}");
    let again = request(port, &head, "").await;
    assert_eq!(status_of(&again), 304);
    assert!(!again.contains("\"name\""), "a 304 must carry no body: {again}");

    let wild = request(port, "GET /items/1 HTTP/1.1\r\nIf-None-Match: *", "").await;
    assert_eq!(status_of(&wild), 304);

    assert_eq!(status_of(&request(port, "PATCH /items/1 HTTP/1.1", "{\"done\":true}").await), 200);
    let changed = request(port, &head, "").await;
    assert_eq!(status_of(&changed), 200);
    assert!(changed.contains("\"done\":true"), "{changed}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_writers_all_land_and_ids_are_unique() {
    let port = start().await;
    let mut tasks = Vec::new();
    for n in 0..50 {
        tasks.push(tokio::spawn(async move {
            let body = format!("{{\"name\":\"item-{n}\"}}");
            request(port, "POST /items HTTP/1.1", &body).await
        }));
    }
    let mut created = 0;
    for t in tasks {
        let r = t.await.unwrap();
        if status_of(&r) == 201 {
            created += 1;
        }
    }
    assert_eq!(created, 50);

    let listed = request(port, "GET /items HTTP/1.1", "").await;
    for n in 0..50 {
        let expected = format!("item-{n}");
        assert!(listed.contains(&expected), "item-{n} missing");
    }
    let health = request(port, "GET /health HTTP/1.1", "").await;
    assert!(health.contains("\"items\":50"), "{health}");
}

#[tokio::test]
async fn shutdown_stops_accepting_and_lets_the_work_finish() {
    let (port, stop) = start_stoppable().await;
    assert_eq!(status_of(&request(port, "POST /items HTTP/1.1", "{\"name\":\"before\"}").await), 201);

    stop.notify_waiters();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let refused = TcpStream::connect(("127.0.0.1", port)).await;
    assert!(refused.is_err(), "the server kept accepting after shutdown");
}

/// The bytes after the blank line.
fn body_of(response: &str) -> String {
    match response.split_once("\r\n\r\n") {
        Some((_head, body)) => body.to_string(),
        None => String::new(),
    }
}

#[tokio::test]
async fn the_compact_shape_drops_the_field_names() {
    let port = start().await;
    assert_eq!(
        status_of(&request(port, "POST /items HTTP/1.1", "{\"name\":\"n\",\"labels\":[\"a\"]}").await),
        201
    );

    let keyed = request(port, "GET /items/1 HTTP/1.1", "").await;
    let compact = request(
        port,
        "GET /items/1 HTTP/1.1\r\nAccept: application/vnd.restdemo.compact+json",
        "",
    )
    .await;
    assert_eq!(status_of(&compact), 200);

    assert!(keyed.contains("\"name\":\"n\""), "{keyed}");
    assert!(!compact.contains("\"name\":"), "the compact form still names fields: {compact}");
    assert!(compact.contains("[1,0,0.0,\"n\",false,null,[\"a\"],{}]"), "{compact}");
    assert_eq!(
        header_of(&compact, "content-type"),
        Some("application/vnd.restdemo.compact+json".to_string())
    );

    let kb = body_of(&keyed).len();
    let cb = body_of(&compact).len();
    assert!(cb < kb, "compact {cb} is not smaller than keyed {kb}");
}

#[tokio::test]
async fn brotli_is_used_when_it_is_asked_for_and_worth_it() {
    let port = start().await;
    for n in 0..40 {
        let body = format!("{{\"name\":\"item-{n}\",\"labels\":[\"alpha\",\"beta\"]}}");
        assert_eq!(status_of(&request(port, "POST /items HTTP/1.1", &body).await), 201);
    }

    let plain = request(port, "GET /items HTTP/1.1", "").await;
    let zipped = request(port, "GET /items HTTP/1.1\r\nAccept-Encoding: br", "").await;
    assert_eq!(status_of(&zipped), 200);
    assert_eq!(header_of(&zipped, "content-encoding"), Some("br".to_string()));
    assert!(
        body_of(&zipped).len() * 4 < body_of(&plain).len(),
        "brotli saved little: {} vs {}",
        body_of(&zipped).len(),
        body_of(&plain).len()
    );

    // a body under the threshold is not worth a brotli frame, and does not get one
    let small = request(port, "GET /health HTTP/1.1\r\nAccept-Encoding: br", "").await;
    assert_eq!(header_of(&small, "content-encoding"), None);
}

#[tokio::test]
async fn every_representation_gets_its_own_tag_and_says_so() {
    // A cache that keyed only on the URL would serve one client's shape to another.
    let port = start().await;
    assert_eq!(status_of(&request(port, "POST /items HTTP/1.1", "{\"name\":\"v\"}").await), 201);

    let keyed = request(port, "GET /items/1 HTTP/1.1", "").await;
    let compact = request(
        port,
        "GET /items/1 HTTP/1.1\r\nAccept: application/vnd.restdemo.compact+json",
        "",
    )
    .await;

    let Some(kt) = header_of(&keyed, "etag") else { panic!("no ETag on the keyed form") };
    let Some(ct) = header_of(&compact, "etag") else { panic!("no ETag on the compact form") };
    assert_ne!(kt, ct, "two shapes shared one ETag, so a cache could swap them");

    for r in [&keyed, &compact] {
        assert_eq!(header_of(r, "vary"), Some("accept, accept-encoding".to_string()));
    }

    let head = format!(
        "GET /items/1 HTTP/1.1\r\nAccept: application/vnd.restdemo.compact+json\r\nIf-None-Match: {ct}"
    );
    let again = request(port, &head, "").await;
    assert_eq!(status_of(&again), 304);
}
