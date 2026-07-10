use crate::dc_bot::serenity::ChannelId;
use poise::serenity_prelude as serenity;

use crate::commands::*;
use std::{collections::HashSet, env, sync::Arc, time::Duration};

use tokio::sync::{
    Mutex, RwLock,
    mpsc::{Receiver, Sender},
};

use mc_rcon::RconClient;
use rust_mc_status::McClient;

use crate::discord_bot::{FromDiscordEvent, FromMinecraftEvent};
use anyhow::Context as any_ctx;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
pub struct Data {
    pub dc_event_tx: Sender<FromDiscordEvent>,
    pub mc_status_client: McClient,
    pub target_channel_id_list: Arc<RwLock<Option<Vec<serenity::ChannelId>>>>,
    pub rcon_client: Arc<Mutex<RconClient>>,
}

// Hardcoded user verification gate
const OWNER_ID: u64 = 1314616785156444175;

pub async fn start_dc_bot(
    mc_event_rx: Receiver<FromMinecraftEvent>,
    dc_event_tx: Sender<FromDiscordEvent>,
    rcon_client: Arc<Mutex<RconClient>>,
) -> anyhow::Result<()> {
    let bridge_channel_list = Arc::new(RwLock::new(None));
    let mc_status_client = McClient::new()
        .with_timeout(Duration::from_secs(5))
        .with_max_parallel(10);

    let data = Data {
        target_channel_id_list: bridge_channel_list.clone(),
        dc_event_tx,
        mc_status_client,
        rcon_client,
    };
    let mut discord_client = init_discord_client(data).await?;

    tokio::spawn({
        listen_for_mc_events(
            mc_event_rx,
            bridge_channel_list,
            Arc::clone(&discord_client.http),
        )
    });

    discord_client
        .start()
        .await
        .with_context(|| format!("Error while starting discord bot"))?;

    Ok(())
}

async fn listen_for_mc_events(
    mut mc_event_rx: Receiver<FromMinecraftEvent>,
    bridge_channel_list: Arc<RwLock<Option<Vec<ChannelId>>>>,
    cache_http: Arc<serenity::Http>,
) {
    while let Some(event) = mc_event_rx.recv().await {
        let formatted_message = format!("**{}**: {}", event.username, event.content);

        // let current_targets = {
        //     let lock = bridge_channel_list.read().await;
        //     lock.clone()
        // };
        let target_channels = bridge_channel_list.read().await.clone();

        if let Some(channels) = target_channels {
            broadcast_to_discord_channels(channels, cache_http.clone(), formatted_message);
        }
    }
}

fn broadcast_to_discord_channels(
    target_channels: Vec<serenity::ChannelId>,
    cache_http: Arc<serenity::Http>,
    message: String,
) {
    let shared_message = Arc::new(message);

    for target_channel in target_channels {
        let http_clone = Arc::clone(&cache_http);
        let msg_clone = Arc::clone(&shared_message);

        tokio::spawn(async move {
            if let Err(why) = target_channel.say(http_clone, &*msg_clone).await {
                eprintln!("Failed to send message to discord channel {target_channel}: {why:?}");
            }
        });
    }
}

async fn init_discord_client(data: Data) -> anyhow::Result<serenity::client::Client> {
    let token = env::var("DISCORD_TOKEN")
        .context("Missing DISCORD_TOKEN environment variable in application environment")?;
    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;
    let framework = init_framework(data);

    let client_builder = serenity::ClientBuilder::new(token, intents).framework(framework);
    let client = client_builder.await?;
    Ok(client)
}

fn init_framework(data: Data) -> poise::Framework<Data, Error> {
    let mut owners = HashSet::new();
    owners.insert(serenity::UserId::new(OWNER_ID));

    let framework: poise::Framework<Data, Error> = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            event_handler: |ctx, event, _, data| Box::pin(event_handler(ctx, event, data)),
            commands: vec![ping(), start_bridge(), stop_bridge(), info(), help()],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("~".into()),
                edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                    Duration::from_secs(3600),
                ))),
                additional_prefixes: vec![
                    poise::Prefix::Literal("hey reze,"),
                    poise::Prefix::Literal("hey reze"),
                ],
                ..Default::default()
            },
            owners,
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                // Register slash commands globally instantly on login setup
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                Ok(data)
            })
        })
        .build();
    framework
}
