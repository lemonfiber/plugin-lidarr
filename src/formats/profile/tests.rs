use lemonfiber_ports::media::Format;
use serde_json::Value;

use super::{hi_res_custom_format, prefers_hi_res, rewrite, set_hi_res_preference, HI_RES_FORMAT};

const HIGH_QUALITY_LOSSY: &str = "High Quality Lossy";
const LOSSLESS: &str = "Lossless";

const PROFILE: &str = r#"{
    "name":"Standard","upgradeAllowed":false,"cutoff":1006,
    "minFormatScore":0,"cutoffFormatScore":0,"formatItems":[],
    "items":[
        {"quality":{"id":0,"name":"Unknown"},"items":[],"allowed":false},
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[
            {"quality":{"id":19,"name":"MP3-VBR-V0"},"items":[],"allowed":false},
            {"quality":{"id":4,"name":"MP3-320"},"items":[],"allowed":false},
            {"quality":{"id":11,"name":"AAC-320"},"items":[],"allowed":false}
        ]},
        {"id":1006,"name":"Lossless","allowed":false,"items":[
            {"quality":{"id":6,"name":"FLAC"},"items":[],"allowed":false},
            {"quality":{"id":7,"name":"ALAC"},"items":[],"allowed":false},
            {"quality":{"id":21,"name":"FLAC 24bit"},"items":[],"allowed":false},
            {"quality":{"id":37,"name":"ALAC 24bit"},"items":[],"allowed":false}
        ]},
        {"quality":{"id":13,"name":"WAV"},"items":[],"allowed":false}
    ]
}"#;

fn rewritten(format: Format) -> Value {
    rewrite(&parsed(PROFILE), format).unwrap_or_default()
}

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_default()
}

fn format_score(profile: &Value, name: &str) -> Option<i64> {
    profile
        .get("formatItems")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(name))
        .and_then(|item| item.get("score"))
        .and_then(Value::as_i64)
}

fn group_allowed(profile: &Value, group: &str) -> bool {
    profile
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(group))
        .and_then(|item| item.get("allowed"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[test]
fn compact_allows_only_high_bitrate_lossy_and_cuts_off_there() {
    let profile = rewritten(Format::Compact);
    assert!(group_allowed(&profile, HIGH_QUALITY_LOSSY));
    assert!(
        !group_allowed(&profile, LOSSLESS),
        "compact does not grab lossless — the cost it avoids"
    );
    assert_eq!(profile.get("cutoff").and_then(Value::as_i64), Some(1005));
}

#[test]
fn lossless_allows_lossy_as_a_fallback_and_cuts_off_at_lossless() {
    let profile = rewritten(Format::Lossless);
    assert!(group_allowed(&profile, HIGH_QUALITY_LOSSY));
    assert!(group_allowed(&profile, LOSSLESS));
    assert_eq!(profile.get("cutoff").and_then(Value::as_i64), Some(1006));
}

#[test]
fn hi_res_shares_the_lossless_allow_axis() {
    let lossless = rewritten(Format::Lossless);
    let hi_res = rewritten(Format::HiRes);
    assert_eq!(lossless.get("items"), hi_res.get("items"));
    assert_eq!(lossless.get("cutoff"), hi_res.get("cutoff"));
}

#[test]
fn nested_leaves_follow_their_group() {
    let profile = rewritten(Format::Lossless);
    let lossless_leaves_allowed = profile
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(LOSSLESS))
        .and_then(|group| group.get("items"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .all(|leaf| leaf.get("allowed").and_then(Value::as_bool) == Some(true));
    assert!(lossless_leaves_allowed);
}

#[test]
fn an_ungrouped_leaf_is_never_wanted() {
    for format in Format::ALL {
        let profile = rewritten(format);
        let stray_allowed = profile
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|item| item.get("name").is_none())
            .any(|leaf| leaf.get("allowed").and_then(Value::as_bool) == Some(true));
        assert!(!stray_allowed, "a format wanted an ungrouped leaf");
    }
}

#[test]
fn every_format_turns_upgrading_on() {
    for format in Format::ALL {
        let profile = rewritten(format);
        assert_eq!(
            profile.get("upgradeAllowed").and_then(Value::as_bool),
            Some(true)
        );
    }
}

#[test]
fn fields_the_rewrite_does_not_touch_are_preserved() {
    let profile = rewritten(Format::Compact);
    assert_eq!(
        profile.get("name").and_then(Value::as_str),
        Some("Standard")
    );
    assert_eq!(
        profile.get("cutoffFormatScore").and_then(Value::as_i64),
        Some(0)
    );
}

#[test]
fn a_cutoff_group_the_profile_lacks_leaves_the_cutoff_as_it_was() {
    const NO_LOSSLESS: &str = r#"{"upgradeAllowed":false,"cutoff":1005,"items":[
        {"id":1005,"name":"High Quality Lossy","allowed":false,"items":[
            {"quality":{"id":4,"name":"MP3-320"},"items":[],"allowed":false}
        ]}
    ]}"#;
    let profile = rewrite(&parsed(NO_LOSSLESS), Format::Lossless).unwrap_or_default();
    assert_eq!(profile.get("cutoff").and_then(Value::as_i64), Some(1005));
}

#[test]
fn a_garbled_profile_is_refused_rather_than_written_back() {
    assert_eq!(rewrite(&Value::Null, Format::Lossless), None);
    assert_eq!(rewrite(&parsed("{}"), Format::Lossless), None);
}

#[test]
fn only_hi_res_prefers_24_bit() {
    assert!(prefers_hi_res(Format::HiRes));
    assert!(!prefers_hi_res(Format::Lossless));
    assert!(!prefers_hi_res(Format::Compact));
}

#[test]
fn the_24_bit_custom_format_matches_a_release_title() {
    let body = hi_res_custom_format();
    assert_eq!(
        body.get("name").and_then(Value::as_str),
        Some(HI_RES_FORMAT)
    );
    let implementation = body
        .get("specifications")
        .and_then(Value::as_array)
        .and_then(|specs| specs.first())
        .and_then(|spec| spec.get("implementation"))
        .and_then(Value::as_str);
    assert_eq!(implementation, Some("ReleaseTitleSpecification"));
}

const WITH_FORMAT: &str = r#"{"cutoffFormatScore":0,"formatItems":[
    {"format":3,"name":"Someone Else's Format","score":25},
    {"format":9,"name":"lemonfiber: 24-bit","score":0}
]}"#;

#[test]
fn preferring_hi_res_scores_the_format_and_the_cutoff() {
    let profile = set_hi_res_preference(&parsed(WITH_FORMAT), true);
    assert!(profile.get("cutoffFormatScore").and_then(Value::as_i64) > Some(0));
    assert!(
        format_score(&profile, HI_RES_FORMAT) > Some(0),
        "the 24-bit format was scored"
    );
    assert_eq!(format_score(&profile, "Someone Else's Format"), Some(25));
}

#[test]
fn not_preferring_hi_res_clears_the_score_so_switching_away_drops_it() {
    let scored = set_hi_res_preference(&parsed(WITH_FORMAT), true);
    let cleared = set_hi_res_preference(&scored, false);
    assert_eq!(
        cleared.get("cutoffFormatScore").and_then(Value::as_i64),
        Some(0)
    );
}

#[test]
fn a_cutoff_score_is_never_raised_beyond_a_format_that_can_reach_it() {
    const NO_FORMAT: &str = r#"{"cutoffFormatScore":0,"formatItems":[]}"#;
    let profile = set_hi_res_preference(&parsed(NO_FORMAT), true);
    assert_eq!(
        profile.get("cutoffFormatScore").and_then(Value::as_i64),
        Some(0)
    );
}

#[test]
fn a_preference_on_a_non_profile_leaves_it_as_it_was() {
    assert_eq!(set_hi_res_preference(&parsed("[]"), true), parsed("[]"));
}

#[test]
fn a_profile_with_no_format_items_has_its_cutoff_score_cleared() {
    let profile = set_hi_res_preference(&parsed(r#"{"cutoffFormatScore":40}"#), true);
    assert_eq!(profile, parsed(r#"{"cutoffFormatScore":0}"#));
}
