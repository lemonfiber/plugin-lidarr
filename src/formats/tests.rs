use std::sync::Arc;

use adapter_kit_servarr::curator::Curator;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::{Method, Request};
use lemonfiber_ports::media::Format;
use lemonfiber_ports::service::{MusicQuality, RootFolder};
use lemonfiber_ports::Client;

use super::Formats;
use crate::adapter::lidarr;

const BASE: &str = "http://lidarr:8686";

const PROFILES: &str = r#"[
    1,
    {"upgradeAllowed":false,"cutoff":1006,"items":[
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[]},
        {"id":1006,"name":"Lossless","allowed":false,"items":[]}
    ]},
    {"id":3,"name":"Broken"},
    {"id":2,"upgradeAllowed":false,"cutoff":1006,"cutoffFormatScore":0,
     "formatItems":[{"format":9,"name":"lemonfiber: 24-bit","score":0}],
     "items":[
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[]},
        {"id":1006,"name":"Lossless","allowed":false,"items":[]}
    ]}
]"#;

fn curator(fake: &Arc<Fake>) -> Curator<Formats> {
    lidarr(
        fake.clone(),
        BASE,
        Arc::new(('a'..='p').collect::<String>()),
    )
}

fn sent(fake: &Fake, method: Method, fragment: &str) -> Vec<Request> {
    fake.requests()
        .into_iter()
        .filter(|one| one.method == method && one.url.contains(fragment))
        .collect()
}

fn profiled(extra: Vec<(Method, &'static str, Answer)>) -> Arc<Fake> {
    let mut routes = extra;
    routes.extend([
        (Method::Get, "/qualityprofile", Answer::reply(200, PROFILES)),
        (Method::Put, "/qualityprofile", Answer::reply(202, "")),
    ]);
    Fake::by_route(routes)
}

#[tokio::test]
async fn a_lossless_choice_rewrites_each_profile_it_can_address_and_leaves_formats_alone() {
    let fake = profiled(Vec::new());
    assert!(curator(&fake)
        .apply_music_format(Format::Lossless)
        .await
        .is_ok());
    let puts = sent(&fake, Method::Put, "/qualityprofile");
    assert_eq!(
        puts.iter().map(|one| one.url.clone()).collect::<Vec<_>>(),
        [format!("{BASE}/api/v1/qualityprofile/2")]
    );
    assert!(puts
        .first()
        .and_then(|one| one.body.as_deref())
        .is_some_and(|body| body.contains(r#""upgradeAllowed":true"#)
            && body.contains(r#""cutoffFormatScore":0"#)));
    assert!(sent(&fake, Method::Get, "/customformat").is_empty());
}

#[tokio::test]
async fn a_hi_res_choice_creates_the_24_bit_format_once_and_prefers_it() {
    let fake = profiled(vec![
        (Method::Get, "/customformat", Answer::reply(200, "[]")),
        (Method::Post, "/customformat", Answer::reply(201, "{}")),
    ]);
    assert!(curator(&fake)
        .apply_music_format(Format::HiRes)
        .await
        .is_ok());
    let created = sent(&fake, Method::Post, "/customformat");
    assert_eq!(created.len(), 1);
    assert!(created
        .first()
        .and_then(|one| one.body.as_deref())
        .is_some_and(|body| body.contains("ReleaseTitleSpecification")));
    assert!(sent(&fake, Method::Put, "/qualityprofile")
        .first()
        .and_then(|one| one.body.as_deref())
        .is_some_and(|body| body.contains(r#""cutoffFormatScore":100"#)));

    let fake = profiled(vec![(
        Method::Get,
        "/customformat",
        Answer::reply(200, r#"[{"id":9,"name":"lemonfiber: 24-bit"}]"#),
    )]);
    assert!(curator(&fake)
        .apply_music_format(Format::HiRes)
        .await
        .is_ok());
    assert!(sent(&fake, Method::Post, "/customformat").is_empty());
}

#[tokio::test]
async fn what_lidarr_will_not_read_or_take_is_a_failure() {
    for fake in [
        Fake::by_route(vec![(
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, "{}"),
        )]),
        Fake::by_route(vec![
            (Method::Get, "/qualityprofile", Answer::reply(200, PROFILES)),
            (Method::Put, "/qualityprofile", Answer::reply(500, "boom")),
        ]),
    ] {
        assert!(curator(&fake)
            .apply_music_format(Format::Lossless)
            .await
            .is_err());
    }
    for fake in [
        Fake::by_route(vec![(
            Method::Get,
            "/customformat",
            Answer::reply(200, "{}"),
        )]),
        Fake::by_route(vec![
            (Method::Get, "/customformat", Answer::reply(200, "[]")),
            (Method::Post, "/customformat", Answer::reply(500, "nope")),
        ]),
    ] {
        assert!(curator(&fake)
            .apply_music_format(Format::HiRes)
            .await
            .is_err());
    }
}

fn music_folder() -> RootFolder {
    RootFolder {
        path: "/data/media/music".to_owned(),
        media_type: "music".to_owned(),
    }
}

#[tokio::test]
async fn a_root_folder_of_music_carries_its_name_and_the_first_profile_of_each_kind() {
    let fake = Fake::by_route(vec![
        (
            Method::Get,
            "/metadataprofile",
            Answer::reply(
                200,
                r#"[{"id":0,"name":"None"},{"id":7,"name":"Standard"}]"#,
            ),
        ),
        (
            Method::Get,
            "/qualityprofile",
            Answer::reply(200, r#"[{"id":4,"name":"Lossless"}]"#),
        ),
        (Method::Post, "/rootfolder", Answer::reply(201, "{}")),
    ]);
    assert!(curator(&fake)
        .register_root_folder(&music_folder())
        .await
        .is_ok());
    let body: serde_json::Value = sent(&fake, Method::Post, "/rootfolder")
        .first()
        .and_then(|one| one.body.as_deref())
        .and_then(|body| serde_json::from_str(body).ok())
        .unwrap_or_default();
    assert_eq!(
        body,
        serde_json::json!({
            "name": "music",
            "path": "/data/media/music",
            "defaultMetadataProfileId": 7,
            "defaultQualityProfileId": 4,
        })
    );
}

#[tokio::test]
async fn a_root_folder_with_no_profile_to_name_carries_its_name_and_path() {
    let fake = Fake::by_route(vec![
        (Method::Get, "/metadataprofile", Answer::reply(200, "[]")),
        (Method::Get, "/qualityprofile", Answer::reply(200, "[]")),
        (Method::Post, "/rootfolder", Answer::reply(201, "{}")),
    ]);
    assert!(curator(&fake)
        .register_root_folder(&music_folder())
        .await
        .is_ok());
    assert!(sent(&fake, Method::Post, "/rootfolder")
        .first()
        .and_then(|one| one.body.as_deref())
        .is_some_and(|body| !body.contains("ProfileId")));
}

#[tokio::test]
async fn a_root_folder_whose_profiles_cannot_be_read_is_not_registered() {
    for fake in [
        Fake::by_route(vec![(
            Method::Get,
            "/metadataprofile",
            Answer::reply(500, ""),
        )]),
        Fake::by_route(vec![
            (Method::Get, "/metadataprofile", Answer::reply(200, "[]")),
            (Method::Get, "/qualityprofile", Answer::reply(500, "")),
        ]),
    ] {
        assert!(curator(&fake)
            .register_root_folder(&music_folder())
            .await
            .is_err());
        assert!(sent(&fake, Method::Post, "/rootfolder").is_empty());
    }
}
