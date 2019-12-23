use crate::v2::{Error, Result};
use serde_json;

/// Manifest version 2 schema 2.
///
/// Specification is at https://docs.docker.com/registry/spec/manifest-v2-2/.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ManifestSchema2Spec {
    #[serde(rename = "schemaVersion")]
    schema_version: u16,
    #[serde(rename = "mediaType")]
    media_type: String,
    config: Config,
    layers: Vec<S2Layer>,
}

/// Super-type for combining a ManifestSchema2 with a ConfigBlob.
#[derive(Debug, Default)]
pub struct ManifestSchema2 {
    pub manifest_spec: ManifestSchema2Spec,
    pub config_blob: ConfigBlob,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub size: u64,
    pub digest: String,
}

/// Partial representation of a container image (application/vnd.docker.container.image.v1+json).
///
/// The remaining fields according to [the image spec v1][image-spec-v1] are not covered.
///
/// [image-spec-v1]: https://github.com/moby/moby/blob/a30990b3c8d0d42280fa501287859e1d2393a951/image/spec/v1.md#image-json-description
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ConfigBlob {
    architecture: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct S2Layer {
    #[serde(rename = "mediaType")]
    media_type: String,
    size: u64,
    digest: String,
    urls: Option<Vec<String>>,
}

/// Manifest List.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ManifestList {
    #[serde(rename = "schemaVersion")]
    schema_version: u16,
    #[serde(rename = "mediaType")]
    media_type: String,
    pub manifests: Vec<ManifestObj>,
}

/// Manifest object.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ManifestObj {
    #[serde(rename = "mediaType")]
    media_type: String,
    size: u64,
    pub digest: String,
    pub platform: Platform,
}

/// Platform-related manifest entries.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Platform {
    pub architecture: String,
    pub os: String,
    #[serde(rename = "os.version")]
    pub os_version: Option<String>,
    #[serde(rename = "os.features")]
    pub os_features: Option<Vec<String>>,
    pub variant: Option<String>,
    pub features: Option<Vec<String>>,
}

impl ManifestSchema2Spec {
    /// Get `Config` object referenced by this manifest.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Fetch the config blob for this manifest
    pub(crate) async fn fetch_config_blob(
        self,
        client: crate::v2::Client,
        repo: String,
    ) -> Result<ManifestSchema2> {
        let url = {
            let ep = format!(
                "{}/v2/{}/blobs/{}",
                client.base_url.clone(),
                repo,
                self.config.digest
            );
            reqwest::Url::parse(&ep).map_err(|e| {
                Error::from(format!("failed to parse url from string '{}': {}", ep, e))
            })?
        };

        let res = client
            .build_reqwest(reqwest::r#async::Client::new().get(url.clone()))
            .send()
            .await
            .map_err(|e| crate::v2::Error::from(format!("{}", e)))?;

        let status = res.status();
        trace!("GET {:?}: {}", url, &status);

        if !status.is_success() {
            return Err(format!("wrong HTTP status '{}'", status).into());
        }

        let config_blob = serde_json::from_slice::<ConfigBlob>(&res.bytes().await?)?;

        Ok(ManifestSchema2 {
            manifest_spec: self,
            config_blob,
        })
    }
}

impl ManifestSchema2 {
    /// List digests of all layers referenced by this manifest.
    ///
    /// The returned layers list is ordered starting with the base image first.
    pub fn get_layers(&self) -> Vec<String> {
        self.manifest_spec
            .layers
            .iter()
            .map(|l| l.digest.clone())
            .collect()
    }

    /// Get the architecture from the config
    pub fn architecture(&self) -> String {
        self.config_blob.architecture.to_owned()
    }
}
