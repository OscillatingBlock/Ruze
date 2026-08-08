use anyhow::Context;
use dotenvy::dotenv;
use std::{env, sync::Arc};
use tokio::sync::Mutex;
use tokio::sync::mpsc::{self};

use mc_rcon::RconClient;

use discord_bot::*;

use crate::mc_log::watch_mc_logs;

use tracing::{debug, info, instrument};

mod discord_bot;
mod event_handler;
mod mc_log;
mod storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();
    info!("Starting Reze Discord Bot...");

    // load env
    dotenv().ok();
    debug!("Environment variables loaded.");

    // setup rcon
    info!("Initializing RCON client...");
    let rcon_client = setup_rcon_client().context("Failed to setup rcon client")?;

    // create communication pipes
    let (mc_event_tx, mc_event_rx) = mpsc::channel::<FromMinecraftEvent>(32);
    let (dc_event_tx, dc_event_rx) = mpsc::channel::<FromDiscordEvent>(32);

    // start minecraft log watcher
    let log_path = env::var("LOG_PATH").context("Error: No LOG_PATH environment variable FOUND")?;
    info!(log_path = %log_path, "Spawning Minecraft log watcher...");
    watch_mc_logs(mc_event_tx, &log_path)
        .await
        .context("Failure while watching Minecraft logs ")?;

    //start discord to minecraft relay
    event_handler::spawn_discord_to_mc_relay(dc_event_rx, Arc::clone(&rcon_client));

    //start discord bot
    info!("Connecting to Discord gateway...");
    dc_bot::start_discord_bot(mc_event_rx, dc_event_tx, rcon_client)
        .await
        .context("Fatal error caught from main Discord bot loop")?;

    info!("Application shutting down cleanly.");
    Ok(())
}

#[instrument(err)]
fn setup_rcon_client() -> anyhow::Result<Arc<Mutex<RconClient>>> {
    // load variables
    let address = env::var("RCON_SERVER_ADDRESS").unwrap_or_else(|_| "localhost:25575".to_string());
    let password =
        env::var("RCON_PASSWORD").context("Expected RCON_PASSWORD in the environment")?;

    // connect to rcon client
    debug!(address = %address, "Establishing TCP connection to RCON server...");
    let client = RconClient::connect(&address)
        .with_context(|| format!("Unable to connect to Minecraft RCON server at {}", address))?;

    // login to rcon client
    client
        .log_in(&password)
        .context("failed to authenticate with RCON server")?;

    info!(address = %address, "Successfully connected and authenticated with Minecraft RCON.");
    Ok(Arc::new(Mutex::new(client)))
}
