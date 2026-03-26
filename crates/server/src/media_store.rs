use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

#[cfg(feature = "s3")]
use aws_sdk_s3::primitives::ByteStream;

/// メディアストレージの抽象インターフェース
/// 将来 S3Store などを追加することで差し替え可能
#[async_trait]
pub trait MediaStore: Send + Sync {
    async fn store(&self, media_id: &str, data: Bytes) -> Result<()>;
    async fn fetch(&self, media_id: &str) -> Result<Option<Bytes>>;
    async fn delete(&self, media_id: &str) -> Result<()>;
}

/// ローカルファイルシステムへの保存実装
pub struct LocalStore {
    base_path: PathBuf,
}

impl LocalStore {
    pub async fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();
        tokio::fs::create_dir_all(&base_path).await?;
        Ok(Self { base_path })
    }

    fn path_for(&self, media_id: &str) -> PathBuf {
        // media_id の先頭2文字をサブディレクトリに（ファイル数分散）
        let prefix = &media_id[..2.min(media_id.len())];
        self.base_path.join(prefix).join(media_id)
    }
}

#[async_trait]
impl MediaStore for LocalStore {
    async fn store(&self, media_id: &str, data: Bytes) -> Result<()> {
        let path = self.path_for(media_id);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = tokio::fs::File::create(&path).await?;
        file.write_all(&data).await?;
        Ok(())
    }

    async fn fetch(&self, media_id: &str) -> Result<Option<Bytes>> {
        let path = self.path_for(media_id);
        match tokio::fs::read(&path).await {
            Ok(data) => Ok(Some(Bytes::from(data))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete(&self, media_id: &str) -> Result<()> {
        let path = self.path_for(media_id);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// S3 互換ストレージへの保存実装
/// 環境変数: S3_BUCKET, AWS_REGION, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY
/// MinIO 等の場合は AWS_ENDPOINT_URL も設定する
#[cfg(feature = "s3")]
pub struct S3Store {
    client: aws_sdk_s3::Client,
    bucket: String,
}

#[cfg(feature = "s3")]
impl S3Store {
    pub async fn new(bucket: impl Into<String>) -> Result<Self> {
        let config = aws_config::load_from_env().await;
        let client = aws_sdk_s3::Client::new(&config);
        Ok(Self {
            client,
            bucket: bucket.into(),
        })
    }
}

#[cfg(feature = "s3")]
#[async_trait]
impl MediaStore for S3Store {
    async fn store(&self, media_id: &str, data: Bytes) -> Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(media_id)
            .body(ByteStream::from(data))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("S3 put error: {e}"))?;
        Ok(())
    }

    async fn delete(&self, media_id: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(media_id)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("S3 delete error: {e}"))?;
        Ok(())
    }

    async fn fetch(&self, media_id: &str) -> Result<Option<Bytes>> {
        let result = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(media_id)
            .send()
            .await;

        match result {
            Ok(output) => {
                let data = output
                    .body
                    .collect()
                    .await
                    .map_err(|e| anyhow::anyhow!("S3 read error: {e}"))?
                    .into_bytes();
                Ok(Some(data))
            }
            Err(e) => {
                let service_err = e.into_service_error();
                if service_err.is_no_such_key() {
                    Ok(None)
                } else {
                    Err(anyhow::anyhow!("S3 get error: {service_err}"))
                }
            }
        }
    }
}
