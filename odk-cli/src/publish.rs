//! Publish a schema to an OCI registry as an artifact.
//!
//! `odk publish` packages a schema's compiled IR as an OCI artifact and pushes
//! it to any registry that speaks the OCI Distribution v2 protocol (Manifold's
//! `/v2/` endpoint, or any other). The schema travels as a single layer under a
//! dedicated artifact type, alongside the OCI-standard empty config object, so
//! any OCI client can pull and inspect it.

use anyhow::{Context, Result, bail};
use odk_schema::CompiledSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// OCI image manifest media type.
const MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
/// OCI "empty" config media type, used for config-less artifacts.
const EMPTY_CONFIG_MEDIA_TYPE: &str = "application/vnd.oci.empty.v1+json";
/// Media type of an ODK schema layer, and the manifest's `artifactType`.
const SCHEMA_ARTIFACT_TYPE: &str = "application/vnd.omni.odk.schema.v1+json";
/// The OCI empty config payload (`{}`).
const EMPTY_CONFIG: &[u8] = b"{}";

/// Where to publish: an OCI repository reference plus optional bearer auth.
pub struct PublishTarget {
    /// Registry base URL, e.g. `https://manifold.omni.dev`.
    pub registry: String,
    /// Repository path, e.g. `myteam/blog-api`.
    pub repository: String,
    /// Tag, e.g. `latest` or a version.
    pub tag: String,
    /// Optional bearer token for authenticated registries.
    pub token: Option<String>,
}

/// Compute an OCI content digest (`sha256:<hex>`) for a blob.
fn sha256_digest(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("sha256:{hex}")
}

/// An OCI content descriptor.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct Descriptor {
    #[serde(rename = "mediaType")]
    media_type: String,
    digest: String,
    size: u64,
}

impl Descriptor {
    fn new(media_type: &str, data: &[u8]) -> Self {
        Self {
            media_type: media_type.to_string(),
            digest: sha256_digest(data),
            size: data.len() as u64,
        }
    }
}

/// Build the OCI image manifest for a schema artifact.
fn build_manifest(config: &Descriptor, layers: &[Descriptor]) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 2,
        "mediaType": MANIFEST_MEDIA_TYPE,
        "artifactType": SCHEMA_ARTIFACT_TYPE,
        "config": config,
        "layers": layers,
    })
}

/// Append the blob digest to an upload location, preserving any existing query.
///
/// The registry returns an opaque upload URL that may already carry query
/// parameters (upload state); the monolithic-upload PUT adds `digest=`.
fn upload_url_with_digest(location: &str, digest: &str) -> String {
    let separator = if location.contains('?') { '&' } else { '?' };
    format!("{location}{separator}digest={digest}")
}

fn base(target: &PublishTarget) -> String {
    format!(
        "{}/v2/{}",
        target.registry.trim_end_matches('/'),
        target.repository
    )
}

/// Format the bearer header value, when a token is configured.
fn bearer(token: &Option<String>) -> Option<String> {
    token.as_ref().map(|token| format!("Bearer {token}"))
}

/// Push a single blob if the registry does not already have it.
fn push_blob(target: &PublishTarget, descriptor: &Descriptor, data: &[u8]) -> Result<()> {
    let base = base(target);
    let auth = bearer(&target.token);

    // skip the upload if the registry already stores this digest
    let mut head = ureq::head(format!("{base}/blobs/{}", descriptor.digest));
    if let Some(auth) = &auth {
        head = head.header("Authorization", auth);
    }
    let head = head
        .config()
        .http_status_as_error(false)
        .build()
        .call()
        .with_context(|| format!("HEAD blob {} failed", descriptor.digest))?;
    if head.status() == 200 {
        return Ok(());
    }

    // start an upload session; the registry returns an opaque upload URL
    let mut start = ureq::post(format!("{base}/blobs/uploads/"));
    if let Some(auth) = &auth {
        start = start.header("Authorization", auth);
    }
    let started = start
        .config()
        .http_status_as_error(false)
        .build()
        .send_empty()
        .context("failed to start blob upload")?;
    if started.status() != 202 {
        bail!(
            "registry rejected blob upload start: HTTP {}",
            started.status()
        );
    }
    let location = started
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .context("blob upload response missing Location header")?
        .to_string();

    // monolithic upload: PUT the bytes with the digest as a query parameter
    let mut put = ureq::put(upload_url_with_digest(&location, &descriptor.digest))
        .header("Content-Type", "application/octet-stream");
    if let Some(auth) = &auth {
        put = put.header("Authorization", auth);
    }
    let done = put
        .config()
        .http_status_as_error(false)
        .build()
        .send(data)
        .context("failed to upload blob")?;
    if done.status() != 201 {
        bail!("registry rejected blob upload: HTTP {}", done.status());
    }
    Ok(())
}

/// Push the manifest, which makes the artifact resolvable by tag.
fn push_manifest(target: &PublishTarget, manifest: &[u8]) -> Result<String> {
    let url = format!("{}/manifests/{}", base(target), target.tag);
    let mut put = ureq::put(&url).header("Content-Type", MANIFEST_MEDIA_TYPE);
    if let Some(auth) = bearer(&target.token) {
        put = put.header("Authorization", auth);
    }
    let response = put
        .config()
        .http_status_as_error(false)
        .build()
        .send(manifest)
        .context("failed to upload manifest")?;
    if response.status() != 201 {
        bail!("registry rejected manifest: HTTP {}", response.status());
    }
    Ok(sha256_digest(manifest))
}

/// Package a schema as an OCI artifact and push it to the target registry.
///
/// Returns the published reference (`registry/repository:tag@digest`).
pub fn publish(schema: &CompiledSchema, target: &PublishTarget) -> Result<String> {
    let schema_json =
        serde_json::to_vec(schema).context("failed to serialize schema to its IR JSON")?;

    let config = Descriptor::new(EMPTY_CONFIG_MEDIA_TYPE, EMPTY_CONFIG);
    let layer = Descriptor::new(SCHEMA_ARTIFACT_TYPE, &schema_json);
    let manifest = build_manifest(&config, std::slice::from_ref(&layer));
    let manifest_bytes =
        serde_json::to_vec(&manifest).context("failed to serialize OCI manifest")?;

    push_blob(target, &config, EMPTY_CONFIG).context("failed to push config blob")?;
    push_blob(target, &layer, &schema_json).context("failed to push schema layer")?;
    let digest = push_manifest(target, &manifest_bytes)?;

    Ok(format!(
        "{}/{}:{}@{}",
        target.registry.trim_end_matches('/'),
        target.repository,
        target.tag,
        digest
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_digest_matches_the_oci_format_and_known_value() {
        // the digest of `{}` is a well-known OCI empty-config constant
        assert_eq!(
            sha256_digest(b"{}"),
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
    }

    #[test]
    fn descriptor_records_media_type_digest_and_size() {
        let descriptor = Descriptor::new("application/json", b"{}");
        assert_eq!(descriptor.media_type, "application/json");
        assert_eq!(descriptor.size, 2);
        assert_eq!(
            descriptor.digest,
            "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
    }

    #[test]
    fn manifest_is_a_v2_artifact_manifest() {
        let config = Descriptor::new(EMPTY_CONFIG_MEDIA_TYPE, EMPTY_CONFIG);
        let layer = Descriptor::new(SCHEMA_ARTIFACT_TYPE, b"{\"name\":\"X\"}");
        let manifest = build_manifest(&config, std::slice::from_ref(&layer));

        assert_eq!(manifest["schemaVersion"], 2);
        assert_eq!(manifest["mediaType"], MANIFEST_MEDIA_TYPE);
        assert_eq!(manifest["artifactType"], SCHEMA_ARTIFACT_TYPE);
        assert_eq!(manifest["config"]["mediaType"], EMPTY_CONFIG_MEDIA_TYPE);
        assert_eq!(manifest["layers"][0]["mediaType"], SCHEMA_ARTIFACT_TYPE);
        assert_eq!(manifest["layers"][0]["digest"], layer.digest);
        assert_eq!(manifest["layers"][0]["size"], 12);
    }

    #[test]
    fn upload_url_appends_digest_preserving_existing_query() {
        assert_eq!(
            upload_url_with_digest("https://r/v2/x/blobs/uploads/abc", "sha256:d"),
            "https://r/v2/x/blobs/uploads/abc?digest=sha256:d"
        );
        assert_eq!(
            upload_url_with_digest("https://r/v2/x/blobs/uploads/abc?state=y", "sha256:d"),
            "https://r/v2/x/blobs/uploads/abc?state=y&digest=sha256:d"
        );
    }

    #[test]
    fn base_url_trims_trailing_slash() {
        let target = PublishTarget {
            registry: "https://manifold.omni.dev/".to_string(),
            repository: "team/api".to_string(),
            tag: "latest".to_string(),
            token: None,
        };
        assert_eq!(base(&target), "https://manifold.omni.dev/v2/team/api");
    }
}
