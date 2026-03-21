use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// メディアストレージの抽象インターフェース
/// 将来 S3Store などを追加することで差し替え可能
#[async_trait]
pub trait MediaStore: Send + Sync {
    async fn store(&self, media_id: &str, data: Bytes) -> Result<()>;
    async fn fetch(&self, media_id: &str) -> Result<Option<Bytes>>;
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
}
