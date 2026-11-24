use serde::{Deserialize, Serialize};

/// Private location of a delivered file. Never serialized into a public API
/// projection or a custom event; only the owning session may resolve it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactLocation {
    #[serde(default = "default_source_kind")]
    pub kind: String,
    pub root: String,
    pub relative_path: String,
}

fn default_source_kind() -> String {
    "workspace-file".to_owned()
}

/// Cheap change detector captured when the file was delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactFingerprint {
    pub size: u64,
    pub modified_at_ns: u64,
}

/// Delivery record persisted next to the assistant message but outside the
/// public message DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRecord {
    pub id: String,
    pub display: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    pub byte_size: u64,
    pub source: ArtifactLocation,
    pub fingerprint: ArtifactFingerprint,
}

impl ArtifactRecord {
    /// Public projection of one record. Actions are a UI hint only; every
    /// operation re-authorizes and re-validates on its own.
    pub fn public_item(&self, requested: &[ArtifactAction]) -> ChatArtifactItem {
        ChatArtifactItem {
            id: self.id.clone(),
            display: self.display.clone(),
            description: self.description.clone(),
            media_type: self.media_type.clone(),
            byte_size: Some(self.byte_size),
            actions: requested
                .iter()
                .copied()
                .filter(|action| match action {
                    ArtifactAction::Preview => {
                        artifact_preview_supported(&self.display, self.media_type.as_deref())
                    }
                    _ => action.supported_by(&self.source.kind),
                })
                .collect(),
        }
    }
}

fn artifact_preview_supported(display: &str, media_type: Option<&str>) -> bool {
    let media_type = media_type
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let media_type = if media_type.is_empty() || media_type == "application/octet-stream" {
        let extension = std::path::Path::new(display)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match extension.as_str() {
            "html" | "htm" => "text/html".to_owned(),
            "pdf" => "application/pdf".to_owned(),
            "png" => "image/png".to_owned(),
            "jpg" | "jpeg" => "image/jpeg".to_owned(),
            "webp" => "image/webp".to_owned(),
            "gif" => "image/gif".to_owned(),
            "svg" => "image/svg+xml".to_owned(),
            "txt" | "log" => "text/plain".to_owned(),
            "md" | "markdown" => "text/markdown".to_owned(),
            "json" => "application/json".to_owned(),
            "xml" => "application/xml".to_owned(),
            "csv" => "text/csv".to_owned(),
            "tsv" => "text/tab-separated-values".to_owned(),
            "yaml" | "yml" => "text/yaml".to_owned(),
            _ => media_type,
        }
    } else {
        media_type
    };

    media_type.starts_with("image/")
        || media_type == "application/pdf"
        || media_type == "text/html"
        || media_type == "application/xhtml+xml"
        || media_type.starts_with("text/")
        || media_type == "application/json"
        || media_type.ends_with("+xml")
}

/// Actions the current client may show. `open`/`reveal` join this list only
/// once a trusted local desktop bridge exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactAction {
    Preview,
    Open,
    Reveal,
    Save,
}

impl ArtifactAction {
    /// Actions every client can use without a native desktop bridge.
    pub const PORTABLE: [Self; 2] = [Self::Preview, Self::Save];

    fn supported_by(self, source_kind: &str) -> bool {
        match self {
            Self::Open | Self::Reveal => source_kind == "workspace-file",
            Self::Preview | Self::Save => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatArtifactItem {
    pub id: String,
    pub display: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ArtifactAction>,
}

/// Public delivery list attached to one finished assistant message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatArtifacts {
    pub version: String,
    pub items: Vec<ChatArtifactItem>,
}

impl ChatArtifacts {
    pub const VERSION: &'static str = "v1";

    /// Build the public projection; `None` when nothing was delivered.
    pub fn from_records(records: &[ArtifactRecord]) -> Option<Self> {
        Self::from_records_with_actions(records, &ArtifactAction::PORTABLE)
    }

    /// Build the projection for the actions a client may actually use. The
    /// host still re-authorizes every operation; this list is a UI hint only.
    pub fn from_records_with_actions(
        records: &[ArtifactRecord],
        actions: &[ArtifactAction],
    ) -> Option<Self> {
        if records.is_empty() {
            return None;
        }
        Some(Self {
            version: Self::VERSION.to_owned(),
            items: records
                .iter()
                .map(|record| record.public_item(actions))
                .collect(),
        })
    }
}
