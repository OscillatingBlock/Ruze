use anyhow::Context;
use linemux::MuxedLines;
use tracing::{error, info};

use regex::Regex;
use std::sync::OnceLock;

use crate::event_handler;
use tokio::sync::mpsc;

use crate::discord_bot::*;

///coordinator to watch Minecraft logs and forward them for processing
pub async fn watch_mc_logs(
    mc_event_tx: mpsc::Sender<FromMinecraftEvent>,
    path: &str,
) -> anyhow::Result<()> {
    let mut watcher =
        MuxedLines::new().context("Failed to initialize MuxedLines background worker")?;

    info!("reading file {path:}");

    watcher
        .add_file(path)
        .await
        .context("Failed to add file to MuxedLines")?;

    tokio::spawn(async move {
        while let Ok(Some(line)) = watcher.next_line().await {
            process_log_line(line.line(), &mc_event_tx).await;
        }
        error!("Minecraft log watcher stream ended.");
    });

    Ok(())
}
// Worker: handles the "Parse -> Relay" data pipeline.
async fn process_log_line(raw_line: &str, tx: &mpsc::Sender<FromMinecraftEvent>) {
    if let Some(event) = parse_log_line(raw_line) {
        event_handler::relay_mc_event_to_discord(tx, event).await;
    }
}

fn parse_log_line(line: &str) -> Option<MinecraftEvent> {
    static CHAT_REGEX: OnceLock<Regex> = OnceLock::new();
    static SYSTEM_REGEX: OnceLock<Regex> = OnceLock::new();

    let chat_re = CHAT_REGEX.get_or_init(|| {
        Regex::new(
            r"^\[\d{2}:\d{2}:\d{2}\]\s\[[^\]]+/INFO\]:\s(?:\[Not Secure\]\s)?<(?P<username>[a-zA-Z0-9_]{3,16})>\s(?P<message>.+)$"
        ).unwrap()
    });

    let sys_re = SYSTEM_REGEX.get_or_init(|| {
        Regex::new(r"^\[\d{2}:\d{2}:\d{2}\]\s\[[^\]]+/INFO\]:\s(?P<payload>.+)$").unwrap()
    });

    // 1. Match Chat Events first
    if let Some(captures) = chat_re.captures(line) {
        let username = captures.name("username")?.as_str().to_string();
        let message = captures.name("message")?.as_str().to_string();
        return Some(MinecraftEvent::Chat { username, message });
    }

    // 2. Process System/Combat/Connection Lines
    if let Some(captures) = sys_re.captures(line) {
        let payload = captures.name("payload")?.as_str();

        // 3. Catch Player Join Events
        if payload.contains("joined the game") {
            return Some(MinecraftEvent::PlayerJoinLeave {
                system_message: payload.to_string(),
                is_join: true,
            });
        }

        // 4. Catch Player Leave / Disconnect Events
        if payload.contains("left the game") {
            return Some(MinecraftEvent::PlayerJoinLeave {
                system_message: payload.to_string(),
                is_join: false,
            });
        }

        if payload.contains("lost connection:") {
            return None;
        }

        if payload.contains("Logged in with entity id")
            || payload.contains("Saving chunks for level")
            || payload.contains("Stopping server")
            || payload.starts_with("Rcon connection from")
        {
            return None;
        }

        if payload.contains("has made the advancement")
            || payload.contains("has completed the challenge")
        {
            return Some(MinecraftEvent::Advancement {
                system_message: payload.to_string(),
            });
        }
        let is_death = DEATH_SENTENCES
            .iter()
            .any(|&sentence| payload.contains(sentence));

        if is_death {
            return Some(MinecraftEvent::Death {
                system_message: payload.to_string(),
            });
        }
    }

    None
}

pub const DEATH_SENTENCES: &[&str] = &[
    "was slain by",
    "was smashed by",
    "was impaled by",
    "was shot by",
    "was pummeled by",
    "was blown up by",
    "was skewered by",
    "was spit at by",
    "was struck by lightning",
    "was frozen to death",
    "was squashed by",
    "was squished too much",
    "was poked to death",
    "was pricked to death",
    "was doomed to fall",
    "fell from a high place",
    "hit the ground too hard",
    "fell out of the world",
    "didn't want to live",
    "experienced kinetic energy",
    "drowned",
    "suffocated in a wall",
    "starved to death",
    "burned to death",
    "went up in flames",
    "tried to swim in lava",
    "discovered the floor was lava",
    "withered away",
    "killed by magic",
    "froze ",
    "left the confines of this world",
];
