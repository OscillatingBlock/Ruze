use anyhow::Context;
use tracing::{info, warn};

pub const DEFAULT_BRIDGE_STORAGE_PATH: &str = "storage.json";

pub async fn load_bridge_channels(path: &str) -> anyhow::Result<Vec<u64>> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => match serde_json::from_str::<Vec<u64>>(&contents) {
            Ok(channels) => Ok(channels),
            Err(err) => {
                warn!(%path, error = %err, "Bridge storage file exists but is corrupt; starting with no bridged channels");
                Ok(Vec::new())
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            info!(%path, "No bridge storage file found; starting with no bridged channels");
            Ok(Vec::new())
        }
        Err(err) => Err(err).with_context(|| format!("Failed to read bridge storage file {path}")),
    }
}

pub async fn save_bridge_channels(path: &str, channels: &[u64]) -> anyhow::Result<()> {
    let json = serde_json::to_string(channels).context("Failed to serialize bridge channels")?;
    tokio::fs::write(path, json)
        .await
        .with_context(|| format!("Failed to write bridge storage file {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_path() -> String {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!(
                "meow_bridge_storage_{}_{}.json",
                std::process::id(),
                n
            ))
            .to_string_lossy()
            .into_owned()
    }

    #[tokio::test]
    async fn save_then_load_round_trips() -> anyhow::Result<()> {
        let path = temp_path();
        let expected = vec![111_111_111u64, 222_222_222, 333_333_333];
        save_bridge_channels(&path, &expected).await?;
        assert_eq!(load_bridge_channels(&path).await?, expected);
        std::fs::remove_file(&path).ok();
        Ok(())
    }

    #[tokio::test]
    async fn save_empty_then_load_returns_empty() -> anyhow::Result<()> {
        let path = temp_path();
        save_bridge_channels(&path, &[]).await?;
        assert_eq!(load_bridge_channels(&path).await?, Vec::<u64>::new());
        std::fs::remove_file(&path).ok();
        Ok(())
    }

    #[tokio::test]
    async fn missing_file_returns_empty() -> anyhow::Result<()> {
        let path = temp_path();
        std::fs::remove_file(&path).ok();
        assert_eq!(load_bridge_channels(&path).await?, Vec::<u64>::new());
        Ok(())
    }

    #[tokio::test]
    async fn corrupt_file_returns_empty() -> anyhow::Result<()> {
        let path = temp_path();
        tokio::fs::write(&path, "not valid json [").await?;
        assert_eq!(load_bridge_channels(&path).await?, Vec::<u64>::new());
        std::fs::remove_file(&path).ok();
        Ok(())
    }
}
