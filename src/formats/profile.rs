use lemonfiber_ports::media::Format;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const HIGH_QUALITY_LOSSY: &str = "High Quality Lossy";

const LOSSLESS: &str = "Lossless";

pub(crate) const HI_RES_FORMAT: &str = "lemonfiber: 24-bit";

const HI_RES_SCORE: i64 = 100;

const HI_RES_TITLE: &str = r"(?i)\b(24[ ._-]?bit|bit[ ._-]?24|FLAC[ ._-]?24)\b";

const fn allowed_groups(format: Format) -> &'static [&'static str] {
    match format {
        Format::Compact => &[HIGH_QUALITY_LOSSY],
        Format::Lossless | Format::HiRes => &[HIGH_QUALITY_LOSSY, LOSSLESS],
    }
}

const fn cutoff_group(format: Format) -> &'static str {
    match format {
        Format::Compact => HIGH_QUALITY_LOSSY,
        Format::Lossless | Format::HiRes => LOSSLESS,
    }
}

pub(crate) const fn prefers_hi_res(format: Format) -> bool {
    matches!(format, Format::HiRes)
}

pub(crate) fn hi_res_custom_format() -> Value {
    serde_json::json!({
        "name": HI_RES_FORMAT,
        "includeCustomFormatWhenRenaming": false,
        "specifications": [{
            "name": "24-bit",
            "implementation": "ReleaseTitleSpecification",
            "negate": false,
            "required": true,
            "fields": [{ "name": "value", "value": HI_RES_TITLE }]
        }]
    })
}

pub(crate) fn set_hi_res_preference(profile: &Value, prefer: bool) -> Value {
    let mut profile = profile.clone();
    let Value::Object(object) = &mut profile else {
        return profile;
    };
    let mut scored = false;
    if let Some(Value::Array(items)) = object.get_mut("formatItems") {
        for entry in items.iter_mut().filter_map(Value::as_object_mut) {
            if entry.get("name").and_then(Value::as_str) == Some(HI_RES_FORMAT) {
                let score = if prefer { HI_RES_SCORE } else { 0 };
                entry.insert("score".to_owned(), score.into());
                scored = prefer;
            }
        }
    }
    let cutoff = if scored { HI_RES_SCORE } else { 0 };
    object.insert("cutoffFormatScore".to_owned(), cutoff.into());
    profile
}

#[derive(Deserialize, Serialize)]
struct Item {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<Value>,
    #[serde(default)]
    items: Vec<Item>,
    allowed: bool,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct Profile {
    #[serde(rename = "upgradeAllowed")]
    upgrade_allowed: bool,
    cutoff: i64,
    items: Vec<Item>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

impl Item {
    fn set_allowed(&mut self, allowed: bool) {
        self.allowed = allowed;
        for nested in &mut self.items {
            nested.allowed = allowed;
        }
    }
}

pub(crate) fn rewrite(profile: &Value, format: Format) -> Option<Value> {
    let mut profile: Profile = serde_json::from_value(profile.clone()).ok()?;
    let allowed = allowed_groups(format);
    let cutoff = cutoff_group(format);
    for item in &mut profile.items {
        let wanted = item
            .name
            .as_deref()
            .is_some_and(|name| allowed.contains(&name));
        item.set_allowed(wanted);
    }
    if let Some(id) = profile
        .items
        .iter()
        .find(|item| item.name.as_deref() == Some(cutoff))
        .and_then(|item| item.id)
    {
        profile.cutoff = id;
    }
    profile.upgrade_allowed = true;
    serde_json::to_value(&profile).ok()
}

#[cfg(test)]
mod tests;
