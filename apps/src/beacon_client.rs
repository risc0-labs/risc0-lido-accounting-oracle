use ethereum_consensus::{
    phase0::SignedBeaconBlockHeader,
    primitives::Root,
    types::mainnet::{BeaconState, SignedBeaconBlock},
    Fork,
};
use reqwest::IntoUrl;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use url::Url;

/// Errors returned by the [BeaconClient].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not parse URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("version field does not match data version")]
    VersionMismatch,
}

/// Response returned by the `get_block_header` API.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetBlockHeaderResponse {
    pub root: Root,
    pub canonical: bool,
    pub header: SignedBeaconBlockHeader,
}

/// Wrapper returned by the API calls.
#[derive(Serialize, Deserialize)]
struct Response<T> {
    data: T,
    #[serde(flatten)]
    meta: HashMap<String, serde_json::Value>,
}

/// Wrapper returned by the API calls that includes a version.
#[derive(Serialize, Deserialize)]
struct VersionedResponse<T> {
    version: Fork,
    #[serde(flatten)]
    inner: Response<T>,
}

/// Simple beacon API client for the `mainnet` preset that can query headers and blocks.
pub struct BeaconClient {
    http: reqwest::Client,
    endpoint: Url,
}

impl BeaconClient {
    /// Creates a new beacon endpoint API client.
    pub fn new<U: IntoUrl>(endpoint: U) -> Result<Self, Error> {
        let client = reqwest::Client::new();
        Ok(Self {
            http: client,
            endpoint: endpoint.into_url()?,
        })
    }

    async fn http_get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let target = self.endpoint.join(path)?;
        let resp = self.http.get(target).send().await?;
        let value = resp.error_for_status()?.json().await?;
        Ok(value)
    }

    /// Retrieves block details for given block id.
    pub async fn get_block(&self, block_id: impl Display) -> Result<SignedBeaconBlock, Error> {
        let path = format!("eth/v2/beacon/blocks/{block_id}");
        let result: VersionedResponse<SignedBeaconBlock> = self.http_get(&path).await?;
        if result.version.to_string() != result.inner.data.version().to_string() {
            return Err(Error::VersionMismatch);
        }
        Ok(result.inner.data)
    }

    pub async fn get_state(&self, state_id: impl Display) -> Result<BeaconState, Error> {
        let path = format!("/eth/v2/debug/beacon/states/{state_id}");
        let result: VersionedResponse<BeaconState> = self.http_get(&path).await?;
        if result.version.to_string() != result.inner.data.version().to_string() {
            return Err(Error::VersionMismatch);
        }
        Ok(result.inner.data)
    }
}
