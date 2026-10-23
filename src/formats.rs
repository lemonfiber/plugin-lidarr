//! What Lidarr answers that the other curators do not: a music format applied to its
//! quality profiles, and the profiles a root folder of music is registered with.

use adapter_kit_servarr::client::Servarr;
use adapter_kit_servarr::curator::Music;
use async_trait::async_trait;
use lemonfiber_ports::http::Method;
use lemonfiber_ports::media::Format;
use lemonfiber_ports::service::{Failure, RootFolder};
use serde::Deserialize;
use serde_json::{Map, Value};

mod profile;

const CUSTOM_FORMATS: &str = "/customformat";

const QUALITY_PROFILES: &str = "/qualityprofile";

const METADATA_PROFILES: &str = "/metadataprofile";

#[derive(Deserialize)]
struct Profiled {
    #[serde(default)]
    id: i64,
}

/// Lidarr's music formats, and the fields a root folder of music carries.
#[derive(Debug, Clone, Copy, Default)]
pub struct Formats;

#[async_trait]
impl Music for Formats {
    async fn apply(&self, servarr: &Servarr, format: Format) -> Result<(), Failure> {
        let prefer = profile::prefers_hi_res(format);
        if prefer {
            hi_res_format_held(servarr).await?;
        }
        let profiles: Vec<Value> = servarr
            .read(QUALITY_PROFILES, "the quality profiles")
            .await?;
        for held in profiles {
            let Some(id) = held.get("id").and_then(Value::as_i64) else {
                continue;
            };
            let Some(rewritten) = profile::rewrite(&held, format) else {
                continue;
            };
            let body = profile::set_hi_res_preference(&rewritten, prefer);
            servarr
                .done(
                    Method::Put,
                    &format!("{QUALITY_PROFILES}/{id}"),
                    Some(body.to_string()),
                )
                .await?;
        }
        Ok(())
    }

    async fn root_folder(
        &self,
        servarr: &Servarr,
        folder: &RootFolder,
    ) -> Result<Map<String, Value>, Failure> {
        let mut fields =
            Map::from_iter([("name".to_owned(), Value::from(folder.media_type.clone()))]);
        let metadata: Vec<Profiled> = servarr
            .read(METADATA_PROFILES, "the metadata profiles")
            .await?;
        if let Some(id) = metadata.iter().map(|one| one.id).find(|id| *id > 0) {
            fields.insert("defaultMetadataProfileId".to_owned(), Value::from(id));
        }
        let quality: Vec<Profiled> = servarr
            .read(QUALITY_PROFILES, "the quality profiles")
            .await?;
        if let Some(id) = quality.iter().map(|one| one.id).find(|id| *id > 0) {
            fields.insert("defaultQualityProfileId".to_owned(), Value::from(id));
        }
        Ok(fields)
    }
}

async fn hi_res_format_held(servarr: &Servarr) -> Result<(), Failure> {
    let held: Vec<Value> = servarr.read(CUSTOM_FORMATS, "the custom formats").await?;
    if held
        .iter()
        .any(|one| one.get("name").and_then(Value::as_str) == Some(profile::HI_RES_FORMAT))
    {
        return Ok(());
    }
    servarr
        .done(
            Method::Post,
            CUSTOM_FORMATS,
            Some(profile::hi_res_custom_format().to_string()),
        )
        .await
}

#[cfg(test)]
mod tests;
