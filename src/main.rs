use anyhow::Context;
use dotenvy::dotenv;
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{self};

use mc_rcon::RconClient;

use discord_bot::*;

use crate::event_handler::handle_from_dc_to_mc;
use crate::mc_log::watch_mc_logs;

mod discord_bot;
mod event_handler;
mod mc_log;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let rcon_client = setup_rcon_client().context("Failed to setup rcon client")?;

    let (mc_event_tx, mc_event_rx) = mpsc::channel::<FromMinecraftEvent>(32);
    let (dc_event_tx, dc_event_rx) = mpsc::channel::<FromDiscordEvent>(32);

    watch_mc_logs(mc_event_tx);
    handle_from_dc_to_mc(dc_event_rx, Arc::clone(&rcon_client));

    dc_bot::start_dc_bot(mc_event_rx, dc_event_tx, rcon_client)
        .await
        .context("Fatal error crash context caught from main Discord bot loop execution hook")?;

    Ok(())
}

fn setup_rcon_client() -> anyhow::Result<Arc<Mutex<RconClient>>> {
    let address = env::var("RCON_SERVER_ADDRESS").unwrap_or_else(|_| "localhost:25575".to_string());
    let client = RconClient::connect(address)
        .map_err(|e| anyhow::anyhow!("Unable to connect to minecraft rcon server: {:?}", e))?;
    let password = env::var("RCON_PASSWORD").expect("Expected RCON_PASSWORD in the environment");
    client
        .log_in(&password)
        .context("failed to authenticate with RCON server")?;

    Ok(Arc::new(Mutex::new(client)))
}
