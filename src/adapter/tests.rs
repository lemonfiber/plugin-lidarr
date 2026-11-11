use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use lemonfiber_contract::adapter::{about_path, ready_path, About};
use lemonfiber_fixtures::http::{Answer, Fake};
use tower::ServiceExt as _;

use super::{about, kit, lidarr, Lidarr, DIGEST, RELEASE, UPSTREAM};

fn a_key() -> String {
    ('e'..='t').collect()
}

const BASE: &str = "http://lidarr:8686";

fn curator(fake: &Arc<Fake>) -> Lidarr {
    lidarr(
        fake.clone(),
        BASE,
        Arc::new(('a'..='p').collect::<String>()),
    )
}

async fn answered_by(
    fake: &Arc<Fake>,
    method: &str,
    path: &str,
    key: Option<&str>,
    body: &str,
) -> (StatusCode, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        request = request.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    let Ok(request) = request.body(Body::from(body.to_owned())) else {
        return (StatusCode::IM_A_TEAPOT, Vec::new());
    };
    let Ok(response) = kit(a_key(), curator(fake)).router().oneshot(request).await;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map(|bytes| bytes.to_vec())
        .unwrap_or_default();
    (status, bytes)
}

async fn answered(
    method: &str,
    path: &str,
    key: Option<&str>,
    body: &str,
) -> (StatusCode, Vec<u8>) {
    answered_by(&Fake::silent(), method, path, key, body).await
}

#[test]
fn it_speaks_the_curator_contracts_and_names_the_release_its_recordings_were_taken_from() {
    let said = about();
    assert_eq!(said.speaks, ["library.curate@1", "music.curate@1"]);
    assert_eq!(said.upstream, UPSTREAM);
    assert_eq!(
        said.releases
            .iter()
            .map(|release| (release.version.as_str(), release.digest.as_str()))
            .collect::<Vec<_>>(),
        [(RELEASE, DIGEST)]
    );
}

#[tokio::test]
async fn about_is_answered_under_the_plugin_s_key() {
    let key = a_key();
    let (status, body) = answered("GET", &about_path(), Some(&key), "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_slice::<About>(&body).ok(), Some(about()));
}

#[tokio::test]
async fn nothing_is_answered_without_the_key_or_under_another() {
    let operation = "/lemonfiber/library.curate/v1/identity";
    for key in [None, Some("not-the-plugin-s-key")] {
        assert_eq!(
            answered("POST", operation, key, "{}").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            answered("GET", &about_path(), key, "").await.0,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
async fn an_unreachable_lidarr_is_answered_as_the_refusal_the_contract_declares() {
    let key = a_key();
    let (status, body) = answered(
        "POST",
        "/lemonfiber/library.curate/v1/identity",
        Some(&key),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let refusal: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
    assert_eq!(
        refusal.and_then(|refusal| refusal.get("type").cloned()),
        Some(serde_json::json!("unavailable"))
    );
}

#[tokio::test]
async fn an_operation_is_answered_from_what_lidarr_says() {
    let key = a_key();
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"appName":"Lidarr","version":"2.14.5"}"#,
    ));
    let (status, body) = answered_by(
        &fake,
        "POST",
        "/lemonfiber/library.curate/v1/identity",
        Some(&key),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).ok(),
        Some(serde_json::json!({ "name": "Lidarr", "version": "2.14.5" }))
    );
    assert!(fake
        .requests()
        .iter()
        .all(|asked| asked.url == format!("{BASE}/api/v1/system/status")));
}

#[tokio::test]
async fn a_capability_it_does_not_speak_is_refused_as_unknown() {
    let key = a_key();
    let (status, _) = answered(
        "POST",
        "/lemonfiber/download.usenet/v1/transfers",
        Some(&key),
        "{}",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn music_is_looked_up_in_lidarr_s_catalogue_over_its_own_contract() {
    let key = a_key();
    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"foreignAlbumId":"9162580e","title":"Abbey Road","artist":{"artistName":"The Beatles"}}]"#,
    ));
    let (status, body) = answered_by(
        &fake,
        "POST",
        "/lemonfiber/music.curate/v1/lookup",
        Some(&key),
        r#"{"unit":"album","term":"abbey road"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
    assert_eq!(
        page.and_then(|page| page.pointer("/found/0/reference").cloned()),
        Some(serde_json::json!("9162580e"))
    );
    assert!(fake.asked_for(&format!("{BASE}/api/v1/album/lookup?term=abbey+road")));
}

#[tokio::test]
async fn it_is_ready_while_lidarr_answers_its_ping() {
    let key = a_key();
    let up = Fake::always(Answer::reply(200, r#"{"status":"OK"}"#));
    let (status, body) = answered_by(&up, "GET", &ready_path(), Some(&key), "").await;
    assert_eq!(
        (status, body.as_slice()),
        (StatusCode::OK, b"true".as_slice())
    );
    assert!(up
        .requests()
        .iter()
        .all(|asked| asked.url == format!("{BASE}/ping")));
    for down in [Fake::silent(), Fake::always(Answer::reply(503, ""))] {
        let (status, body) = answered_by(&down, "GET", &ready_path(), Some(&key), "").await;
        assert_eq!(
            (status, body.as_slice()),
            (StatusCode::OK, b"false".as_slice())
        );
    }
}

#[tokio::test]
async fn unqueue_and_grab_are_unsupported_and_open_to_no_key_but_the_plugin_s() {
    for (operation, body) in [
        ("unqueue", r#"{"kind":"tv","id":51,"blocklist":true}"#),
        ("grab", r#"{"kind":"movies","id":7,"release":"0000"}"#),
    ] {
        let path = format!("/lemonfiber/library.curate/v1/{operation}");
        let fake = Fake::always(Answer::reply(200, "{}"));
        for key in [None, Some("not-the-plugin-s-key")] {
            assert_eq!(
                answered_by(&fake, "POST", &path, key, body).await.0,
                StatusCode::UNAUTHORIZED,
                "{operation}"
            );
        }
        let (_, said) = answered_by(&fake, "POST", &path, Some(&a_key()), body).await;
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&said)
                .ok()
                .and_then(|refusal| refusal.get("type").cloned()),
            Some(serde_json::json!("unsupported")),
            "{operation}"
        );
        assert!(fake.requests().is_empty(), "{operation}");
    }
}

#[tokio::test]
async fn an_album_is_followed_over_library_curate_by_lidarr_s_own_album_id() {
    let key = a_key();
    let fake = Fake::always(Answer::reply(
        200,
        r#"[{"id":7,"title":"Kind of Blue","releaseDate":"1959-08-17T00:00:00Z","monitored":true}]"#,
    ));
    let (status, body) = answered_by(
        &fake,
        "POST",
        "/lemonfiber/library.curate/v1/find_items",
        Some(&key),
        r#"{"kind":"music","term":"kind of blue"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).ok(),
        Some(serde_json::json!([
            { "id": 7, "title": "Kind of Blue", "year": 1959, "monitored": true }
        ]))
    );
    assert!(fake
        .requests()
        .iter()
        .all(|asked| asked.url == format!("{BASE}/api/v1/album")));
}

#[tokio::test]
async fn books_and_telling_the_core_are_unsupported_and_ask_lidarr_nothing() {
    let key = a_key();
    for (operation, body) in [
        ("library", r#"{"kind":"books"}"#),
        (
            "events",
            r#"{"address":"https://core.invalid","pin":"00","key":"00"}"#,
        ),
    ] {
        let fake = Fake::always(Answer::reply(200, "{}"));
        let path = format!("/lemonfiber/library.curate/v1/{operation}");
        let (_, said) = answered_by(&fake, "POST", &path, Some(&key), body).await;
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&said)
                .ok()
                .and_then(|refusal| refusal.get("type").cloned()),
            Some(serde_json::json!("unsupported")),
            "{operation}"
        );
        assert!(fake.requests().is_empty(), "{operation}");
    }
}
