use lemonfiber_ports::http::{Http, Method, Request};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::{method, read_as_text, Web};

async fn answering(answer: &'static str) -> Option<String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let address = listener.local_addr().ok()?;
    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut asked = [0_u8; 4096];
            let _ = stream.read(&mut asked).await;
            let _ = stream.write_all(answer.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
    });
    Some(format!("http://{address}/"))
}

fn asking(url: String) -> Request {
    Request {
        method: Method::Get,
        url,
        headers: vec![("Accept".to_owned(), "application/json".to_owned())],
        body: None,
        pinned: None,
    }
}

#[tokio::test]
async fn an_answer_is_read_with_its_status_headers_and_body() {
    let url = answering(
        "HTTP/1.1 404 Not Found\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n{}",
    )
    .await;
    let read = match (url, Web::new()) {
        (Some(url), Ok(web)) => web.send(&asking(url)).await.ok(),
        _ => None,
    };
    let read = read.map(|response| {
        (
            response.status,
            response.header("Content-Type").map(str::to_owned),
            response.body,
        )
    });
    assert_eq!(
        read,
        Some((404, Some("application/json".to_owned()), "{}".to_owned()))
    );
}

#[tokio::test]
async fn a_redirect_is_answered_rather_than_followed() {
    let url = answering(
        "HTTP/1.1 302 Found\r\nlocation: http://192.0.2.1/elsewhere\r\ncontent-length: 0\r\n\r\n",
    )
    .await;
    let status = match (url, Web::new()) {
        (Some(url), Ok(web)) => web.send(&asking(url)).await.ok().map(|read| read.status),
        _ => None,
    };
    assert_eq!(status, Some(302));
}

#[tokio::test]
async fn a_body_past_what_was_asked_for_is_held_as_no_body() {
    let url = answering("HTTP/1.1 200 OK\r\ncontent-length: 10\r\n\r\n0123456789").await;
    let fetched = match (url, Web::new()) {
        (Some(url), Ok(web)) => web.fetch(&asking(url), 4).await.ok(),
        _ => None,
    };
    assert_eq!(
        fetched.map(|fetched| (fetched.status, fetched.bytes)),
        Some((200, None))
    );
}

#[tokio::test]
async fn nothing_listening_is_unreachable() {
    let address = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener.local_addr().ok(),
        Err(_) => None,
    };
    let reached = match (address, Web::new()) {
        (Some(address), Ok(web)) => Some(
            web.send(&asking(format!("http://{address}/")))
                .await
                .is_err(),
        ),
        _ => None,
    };
    assert_eq!(reached, Some(true));
}

const BODY: &str = r#"{"Name":"admin"}"#;

#[tokio::test]
async fn a_body_is_sent_as_it_was_written() {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0").await else {
        return;
    };
    let address = listener.local_addr().ok();
    let heard = tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else {
            return String::new();
        };
        let mut heard = Vec::new();
        let mut chunk = [0_u8; 1024];
        while !heard.ends_with(BODY.as_bytes()) {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(read) => heard.extend_from_slice(chunk.get(..read).unwrap_or_default()),
            }
        }
        let _ = stream
            .write_all(b"HTTP/1.1 204 No Content\r\ncontent-length: 0\r\n\r\n")
            .await;
        String::from_utf8_lossy(&heard).into_owned()
    });
    let sent = match (address, Web::new()) {
        (Some(address), Ok(web)) => web
            .send(&Request {
                method: Method::Post,
                url: format!("http://{address}/Startup/User"),
                headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
                body: Some(BODY.to_owned()),
                pinned: None,
            })
            .await
            .ok()
            .map(|answer| answer.status),
        _ => None,
    };
    let heard = heard.await.unwrap_or_default();
    assert_eq!(sent, Some(204));
    assert!(heard.starts_with("POST /Startup/User"), "{heard}");
    assert!(heard.ends_with(BODY), "{heard}");
}

#[test]
fn every_method_is_sent_as_itself() {
    assert_eq!(
        [
            Method::Get,
            Method::Post,
            Method::Put,
            Method::Patch,
            Method::Delete
        ]
        .map(method),
        [
            reqwest::Method::GET,
            reqwest::Method::POST,
            reqwest::Method::PUT,
            reqwest::Method::PATCH,
            reqwest::Method::DELETE,
        ]
    );
}

#[tokio::test]
async fn a_text_answer_past_the_bound_is_no_answer_at_all() {
    let url = answering("HTTP/1.1 200 OK\r\ncontent-length: 10\r\n\r\n0123456789").await;
    let read = match (url, Web::new()) {
        (Some(url), Ok(web)) => Some(read_as_text(&web.client, &asking(url), 4).await.is_err()),
        _ => None,
    };
    assert_eq!(read, Some(true));
}
