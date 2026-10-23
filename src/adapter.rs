//! What this adapter says it is, and the kit that serves its contract.

use std::sync::Arc;

use adapter_kit_servarr::client::Servarr;
use adapter_kit_servarr::curator::{Curator, Files};
use lemonfiber_adapter_kit::credential::Holding;
use lemonfiber_adapter_kit::{Kit, Readiness};
use lemonfiber_contract::adapter::{About, Release};
use lemonfiber_contract::capabilities::library;
use lemonfiber_ports::http::Http;

use crate::formats::Formats;

/// Where Lidarr answers, by its service id and the port it listens on.
pub const ADDRESS: &str = "http://lidarr:8686";

/// The upstream this adapter fronts, as its project names itself.
pub const UPSTREAM: &str = "Lidarr";

/// The Lidarr release this adapter's recordings were taken from.
pub const RELEASE: &str = "2.14.5.4836-ls9";

/// The digest of the image that release's recordings were taken from.
pub const DIGEST: &str = "sha256:5e1235d00b5d1c1f60ca0d472e554a6611aef41aa7b5b6d88260214bf4809af0";

/// What every failure is blamed on.
pub const SERVICE: &str = "lidarr";

/// The version of Lidarr's API this adapter speaks.
pub const API_VERSION: u32 = 1;

/// What this adapter says it is: the contract it speaks and the release it supports.
#[must_use]
pub fn about() -> About {
    About {
        speaks: vec![lemonfiber_contract::spoken(
            library::curate::CAPABILITY,
            library::curate::MAJOR,
        )],
        upstream: UPSTREAM.to_owned(),
        releases: vec![Release {
            version: RELEASE.to_owned(),
            digest: DIGEST.to_owned(),
        }],
    }
}

/// The Lidarr reached at `base` over `http`, curating music with the key `held`
/// supplies.
#[must_use]
pub fn lidarr(
    http: Arc<dyn Http>,
    base: impl Into<String>,
    held: Arc<dyn Holding>,
) -> Curator<Formats> {
    Curator::new(
        Servarr::new(http, base, held, SERVICE, API_VERSION),
        Files::MUSIC,
        Formats,
    )
}

/// The contract, served from `lidarr` under `key`, ready while Lidarr answers.
#[must_use]
pub fn kit(key: String, lidarr: Arc<Curator<Formats>>) -> Kit {
    let asked = Arc::clone(&lidarr);
    let ready: Readiness = Arc::new(move || {
        let asked = Arc::clone(&asked);
        Box::pin(async move { asked.servarr().answers().await })
    });
    Kit::new(key, about())
        .ready_when(ready)
        .serving(library::curate::served(lidarr))
}

#[cfg(test)]
mod tests;
