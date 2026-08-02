use async_trait::async_trait;
use std::path::Path;

use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region, SharedHttpClient};
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_smithy_http_client::tls::{self, TrustStore};
use aws_smithy_types::retry::RetryConfig;
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use tokio::io::AsyncReadExt;

use super::traits::{BackendCapabilities, Capability, StorageBackend, StorageError};
use crate::background::config::S3Config;

const S3_KEY_ENCODE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'-')
    .remove(b'_')
    .remove(b'.');

pub const DEFAULT_MULTIPART_THRESHOLD: u64 = 8 * 1024 * 1024;
pub const MIN_PART_SIZE: u64 = 5 * 1024 * 1024;
pub const MAX_PART_COUNT: u64 = 10_000;

pub fn plan_part_size(size: u64) -> u64 {
    let by_count = size.div_ceil(MAX_PART_COUNT);
    by_count.max(MIN_PART_SIZE)
}

pub struct S3Backend {
    client: aws_sdk_s3::Client,
    config: S3Config,
}

impl S3Backend {
    pub fn new(config: S3Config) -> Result<Self, StorageError> {
        let http_client = build_http_client(&config)?;

        let credentials = Credentials::new(
            config.access_key_id.clone(),
            config.secret_access_key.clone(),
            config.session_token.clone(),
            None,
            "static",
        );

        let mut builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .endpoint_url(&config.endpoint)
            .credentials_provider(credentials)
            .force_path_style(config.path_style)
            .http_client(http_client)
            .retry_config(RetryConfig::disabled());

        if let Some(region) = &config.region {
            builder = builder.region(Region::new(region.clone()));
        }

        let client = aws_sdk_s3::Client::from_conf(builder.build());
        Ok(Self { client, config })
    }

    pub fn object_key(&self, path: &str) -> String {
        let trimmed = path.trim_start_matches('/');
        if self.config.path_prefix.is_empty() {
            trimmed.to_string()
        } else {
            format!(
                "{}/{}",
                self.config.path_prefix.trim_end_matches('/'),
                trimmed
            )
        }
    }

    pub fn copy_source(&self, key: &str) -> String {
        format!(
            "{}/{}",
            self.config.bucket,
            utf8_percent_encode(key, S3_KEY_ENCODE)
        )
    }

    pub fn multipart_threshold(&self) -> u64 {
        self.config
            .multipart_threshold_bytes
            .unwrap_or(DEFAULT_MULTIPART_THRESHOLD)
    }

    async fn single_upload(&self, source_path: &Path, key: &str) -> Result<(), StorageError> {
        let body = ByteStream::from_path(source_path)
            .await
            .map_err(|e| StorageError::Io(std::io::Error::other(e)))?;

        self.client
            .put_object()
            .bucket(&self.config.bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| self.map_sdk_error(e, "upload", key))
    }

    async fn upload_parts(
        &self,
        source_path: &Path,
        key: &str,
        upload_id: &str,
        part_size: u64,
    ) -> Result<Vec<CompletedPart>, StorageError> {
        let mut file = tokio::fs::File::open(source_path)
            .await
            .map_err(StorageError::Io)?;
        let mut completed = Vec::new();
        let mut buf = vec![0u8; part_size as usize];
        let mut part_number: i32 = 1;

        loop {
            let n = file.read(&mut buf).await.map_err(StorageError::Io)?;
            if n == 0 {
                break;
            }
            buf.truncate(n);
            let body = ByteStream::from(std::mem::take(&mut buf));
            buf.resize(part_size as usize, 0);

            let resp = self
                .client
                .upload_part()
                .bucket(&self.config.bucket)
                .key(key)
                .upload_id(upload_id)
                .part_number(part_number)
                .body(body)
                .send()
                .await
                .map_err(|e| self.map_sdk_error(e, "upload part", key))?;

            let etag = resp
                .e_tag()
                .ok_or_else(|| {
                    StorageError::Transfer(format!(
                        "S3 upload part {} returned no etag for '{}'",
                        part_number, key
                    ))
                })?
                .to_string();
            completed.push(
                CompletedPart::builder()
                    .e_tag(etag)
                    .part_number(part_number)
                    .build(),
            );
            part_number += 1;
        }

        Ok(completed)
    }

    async fn multipart_upload(
        &self,
        source_path: &Path,
        key: &str,
        size: u64,
    ) -> Result<(), StorageError> {
        let part_size = plan_part_size(size);

        let create = self
            .client
            .create_multipart_upload()
            .bucket(&self.config.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| self.map_sdk_error(e, "create multipart upload", key))?;
        let upload_id = create.upload_id().ok_or_else(|| {
            StorageError::Transfer(format!(
                "S3 create multipart upload returned no upload id for '{}'",
                key
            ))
        })?;

        let parts = match self.upload_parts(source_path, key, upload_id, part_size).await {
            Ok(parts) => parts,
            Err(err) => {
                let _ = self
                    .client
                    .abort_multipart_upload()
                    .bucket(&self.config.bucket)
                    .key(key)
                    .upload_id(upload_id)
                    .send()
                    .await;
                return Err(err);
            }
        };

        let completed = parts.iter().fold(
            CompletedMultipartUpload::builder(),
            |builder, part| builder.parts(part.clone()),
        );
        self.client
            .complete_multipart_upload()
            .bucket(&self.config.bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(completed.build())
            .send()
            .await
            .map(|_| ())
            .map_err(|e| self.map_sdk_error(e, "complete multipart upload", key))
    }

    fn map_sdk_error<E>(&self, err: aws_sdk_s3::error::SdkError<E>, op: &str, key: &str) -> StorageError
    where
        E: std::error::Error + ProvideErrorMetadata,
    {
        match err {
            aws_sdk_s3::error::SdkError::ServiceError(se) => {
                let code = se.err().meta().code().unwrap_or("").to_string();
                let status = se.raw().status().as_u16();
                let message = se.err().meta().message().unwrap_or("").to_string();
                match status {
                    301 => StorageError::Config(format!(
                        "S3 {} redirected for '{}': {}",
                        op, key, code
                    )),
                    403 => StorageError::PermissionDenied(format!(
                        "S3 {} access denied for '{}': {}",
                        op, key, message
                    )),
                    404 => StorageError::NotFound(format!(
                        "S3 {} not found '{}': {}",
                        op, key, message
                    )),
                    400 if code == "InvalidAccessKeyId" || code == "SignatureDoesNotMatch" => {
                        StorageError::Auth(format!(
                            "S3 {} authentication failed for '{}': {}",
                            op, key, code
                        ))
                    }
                    5.. => StorageError::Transfer(format!(
                        "S3 {} server error {} for '{}': {}",
                        op, status, key, message
                    )),
                    _ => StorageError::Transfer(format!(
                        "S3 {} failed for '{}' ({}): {}",
                        op, key, code, message
                    )),
                }
            }
            aws_sdk_s3::error::SdkError::ConstructionFailure(_) => StorageError::Config(format!(
                "S3 {} request construction failed for '{}'",
                op, key
            )),
            aws_sdk_s3::error::SdkError::TimeoutError(_) | aws_sdk_s3::error::SdkError::DispatchFailure(_) => {
                StorageError::Connection(format!("S3 {} timed out for '{}'", op, key))
            }
            aws_sdk_s3::error::SdkError::ResponseError(_) => StorageError::Transfer(format!(
                "S3 {} malformed response for '{}'",
                op, key
            )),
            _ => StorageError::Transfer(format!("S3 {} unknown error for '{}'", op, key)),
        }
    }
}

fn build_http_client(config: &S3Config) -> Result<SharedHttpClient, StorageError> {
    let tls_context = if let Some(ca_pem) = &config.ca_cert_pem {
        let trust_store = TrustStore::empty().with_pem_certificate(ca_pem.as_bytes().to_vec());
        tls::TlsContext::builder()
            .with_trust_store(trust_store)
            .build()
            .map_err(|e| StorageError::Config(format!("failed to build TLS context: {}", e)))?
    } else {
        tls::TlsContext::default()
    };

    let client = aws_smithy_http_client::Builder::new()
        .tls_provider(tls::Provider::Rustls(
            tls::rustls_provider::CryptoMode::Ring,
        ))
        .tls_context(tls_context)
        .build_https();

    Ok(client)
}

#[async_trait]
impl StorageBackend for S3Backend {
    async fn upload(&self, source_path: &Path, dest_path: &str) -> Result<(), StorageError> {
        let key = self.object_key(dest_path);
        let size = tokio::fs::metadata(source_path)
            .await
            .map_err(StorageError::Io)?
            .len();

        if size <= self.multipart_threshold() {
            self.single_upload(source_path, &key).await
        } else {
            self.multipart_upload(source_path, &key, size).await
        }
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let key = self.object_key(path);
        self.client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&key)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| self.map_sdk_error(e, "delete", &key))
    }

    fn name(&self) -> &str {
        "s3"
    }

    fn capabilities(&self) -> Option<&dyn BackendCapabilities> {
        Some(self)
    }
}

#[async_trait]
impl BackendCapabilities for S3Backend {
    fn supports(&self, capability: Capability) -> bool {
        matches!(capability, Capability::Rename)
    }

    async fn rename(&self, old_path: &str, new_path: &str) -> Result<(), StorageError> {
        let old_key = self.object_key(old_path);
        let new_key = self.object_key(new_path);

        self.client
            .copy_object()
            .bucket(&self.config.bucket)
            .key(&new_key)
            .copy_source(self.copy_source(&old_key))
            .send()
            .await
            .map_err(|e| self.map_sdk_error(e, "rename", &old_key))?;

        self.delete(old_path).await
    }
}


