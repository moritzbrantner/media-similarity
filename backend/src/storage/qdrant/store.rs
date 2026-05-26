use std::fmt;
use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::domain::models::{FacePointPayload, ImagePayload};
use crate::storage::{MediaSearchFilter, MediaVectorStore, ScoredPoint, StoredPoint};

const EXPECTED_DISTANCE: &str = "Cosine";
const VISUAL_VECTOR_NAME: &str = "visual";
const FACE_VECTOR_NAME: &str = "face";
const MAX_RETRY_BACKOFF_MS: u64 = 1_000;

#[derive(Clone, Debug, PartialEq)]
pub struct QdrantHttpOptions {
    pub request_timeout_ms: u64,
    pub connect_timeout_ms: u64,
    pub retry_attempts: u32,
    pub retry_backoff_ms: u64,
}

impl Default for QdrantHttpOptions {
    fn default() -> Self {
        Self {
            request_timeout_ms: 30_000,
            connect_timeout_ms: 2_000,
            retry_attempts: 2,
            retry_backoff_ms: 100,
        }
    }
}

#[derive(Clone)]
pub struct QdrantImageStore {
    client: Client,
    base_urls: Vec<String>,
    collection: String,
    visual_vector_size: usize,
    face_vector_size: usize,
    http_options: QdrantHttpOptions,
}
