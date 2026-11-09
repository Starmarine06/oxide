//! Removes ads from player responses before the page sees them, skips any
//! ad that still slips through and hides upsell banners. Runs entirely in
//! the page, so it works the same on every OS webview.

use super::Plugin;

pub struct AdBlocker;

impl Plugin for AdBlocker {
    fn id(&self) -> &'static str {
        "ad-blocker"
    }
    fn name(&self) -> &'static str {
        "Ad blocker"
    }
    fn description(&self) -> &'static str {
        "Remove audio and video ads and Premium banners"
    }
    fn defaults(&self) -> serde_json::Value {
        serde_json::json!({ "enabled": true })
    }
    fn script(&self) -> Option<&'static str> {
        Some(include_str!("../../scripts/adblock.js"))
    }
}
