use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use crate::discord_bot::*;
use mc_rcon::RconClient;

pub fn handle_from_dc_to_mc(
    mut dc_event_rx: mpsc::Receiver<FromDiscordEvent>,
    rcon_clone: Arc<Mutex<RconClient>>,
) {
    tokio::spawn(async move {
        while let Some(event) = dc_event_rx.recv().await {
            let formatted_command = format!(
                r#"tellraw @a {{"text":"[Discord] <{}>: {}", "color":"gold"}}"#,
                event.username, event.content
            );
            let guard = rcon_clone.lock().await;
            if let Err(why) = guard.send_command(&formatted_command) {
                println!("failed to send command to rcon server: {why:?}")
            }
        }
    });
}

pub async fn handle_from_mc_to_dc(
    mc_event_tx: &mpsc::Sender<FromMinecraftEvent>,
    event: MinecraftEvent,
) {
    let discord_payload = match event {
        MinecraftEvent::Chat { username, message } => FromMinecraftEvent {
            username,
            content: message,
        },
        MinecraftEvent::Death { system_message } => {
            let bold_msg = bold_first_word(&system_message);
            FromMinecraftEvent {
                username: "⚰️".to_string(),
                content: bold_msg,
            }
        }
        MinecraftEvent::Advancement { system_message } => {
            let bold_msg = bold_first_word(&system_message);
            FromMinecraftEvent {
                username: "🏆".to_string(),
                content: bold_msg,
            }
        }
        MinecraftEvent::PlayerJoinLeave {
            system_message,
            is_join,
        } => {
            let icon = if is_join { "🟢 " } else { "🔴 " };
            let bold_msg = bold_first_word(&system_message);
            FromMinecraftEvent {
                username: icon.to_string(),
                content: bold_msg,
            }
        }
    };

    if let Err(why) = mc_event_tx.send(discord_payload).await {
        println!("failed to send FromMinecraftEvent: {why:?}");
    }
}

fn bold_first_word(text: &str) -> String {
    if let Some((first_word, rest)) = text.split_once(' ') {
        format!("**{}** {}", first_word, rest)
    } else {
        format!("**{}**", text)
    }
}
