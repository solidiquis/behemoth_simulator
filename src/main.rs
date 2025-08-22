use anyhow::{Context, Result};
use clap::{Parser, crate_description, crate_name, crate_version};
use rand::seq::IndexedRandom;
use sift_stream::{
    ChannelConfig, ChannelDataType, ChannelValue, Credentials, Flow, FlowConfig,
    IngestionConfigForm, RecoveryStrategy, RunForm, SiftStreamBuilder, TimeValue,
};
use std::{
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tokio::{runtime, signal, time::sleep};

fn main() -> ExitCode {
    let clargs = Clargs::parse();

    tracing_subscriber::fmt()
        .with_env_filter("sift_stream=info,info")
        .init();

    let runtime = runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime");

    let program_result = runtime.block_on(async move { run(clargs).await });

    match program_result {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err:?}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Parser)]
#[command(
    version = crate_version!(),
    about = crate_description!(),
)]
struct Clargs {
    /// Asset name
    #[arg(short, long, default_value_t = String::from(crate_name!()))]
    asset: String,

    /// The number of components the asset has
    #[arg(short, long, default_value_t = 100)]
    num_components: usize,

    /// The number of channels the asset has
    #[arg(short, long, default_value_t = 10)]
    channels_per_component: usize,

    /// The desired frequency in which to send data in Hz
    #[arg(short, long, default_value_t = 1000)]
    frequency: usize,

    /// Sift API key
    #[arg(short = 'k', long, group = "creds")]
    apikey: String,

    /// Sift gRPC URL (http/https must be included)
    #[arg(short, long, requires = "creds")]
    uri: String,

    /// Disables TLS for environments that don't use it
    #[arg(short, long)]
    disable_tls: bool,
}

async fn run(args: Clargs) -> Result<()> {
    let Clargs {
        num_components,
        channels_per_component,
        frequency,
        uri,
        apikey,
        disable_tls,
        asset,
    } = args;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("failed to get system time")?;
    let flow_name = format!("behemoth.{num_components}.{channels_per_component}.{frequency}");
    let run_name = format!("{asset}.{}", now.as_millis());
    let mut channel_configs = Vec::with_capacity(num_components * channels_per_component);

    for i in 0..num_components {
        for j in 0..channels_per_component {
            channel_configs.push(ChannelConfig {
                name: format!("sensor{i}.channel{j}"),
                data_type: ChannelDataType::Int64.into(),
                ..Default::default()
            });
        }
    }

    let flow_config = FlowConfig {
        name: flow_name.clone(),
        channels: channel_configs.clone(),
    };
    let ingestion_config = IngestionConfigForm {
        asset_name: asset.clone(),
        client_key: asset.clone(),
        flows: vec![flow_config],
    };
    let run = RunForm {
        name: run_name.clone(),
        client_key: run_name,
        description: None,
        tags: None,
    };
    let credentials = Credentials::Config { uri, apikey };

    let mut builder = SiftStreamBuilder::new(credentials)
        .recovery_strategy(RecoveryStrategy::default())
        .attach_run(run)
        .ingestion_config(ingestion_config);

    builder = {
        if disable_tls {
            builder.disable_tls()
        } else {
            builder
        }
    };

    let mut sift_stream = builder
        .build()
        .await
        .context("failed to build sift stream")?;

    let mut rng = rand::rng();

    let values: Vec<i64> = (1..100).collect();

    // cycle through these
    let mut message_pool = Vec::with_capacity(100);

    for _ in 0..message_pool.capacity() {
        let message = channel_configs
            .iter()
            .map(|c| ChannelValue::new(&c.name, *values.choose(&mut rng).unwrap()))
            .collect::<Vec<_>>();

        message_pool.push(message);
    }
    let mut messages = message_pool.iter().cycle();

    let terminate_stream = Arc::new(AtomicBool::default());

    let terminate = terminate_stream.clone();
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        tracing::info!("SIGINT - terminating stream");
        terminate.store(true, Ordering::Relaxed);
    });

    let delay_per_message =
        Duration::from_nanos((1.0 / (frequency as f64) * 10.0_f64.powi(9)).ceil() as u64);

    while !terminate_stream.load(Ordering::Relaxed) {
        let msg_tx_start = Instant::now();

        let message = messages.next().unwrap();

        sift_stream
            .send(Flow::new(&flow_name, TimeValue::now(), message))
            .await
            .context("error while sending message")?;

        let duration_delta = msg_tx_start.elapsed();

        if duration_delta < delay_per_message {
            sleep(delay_per_message - duration_delta).await
        }
    }
    sift_stream
        .finish()
        .await
        .context("error terminating sift stream")?;

    Ok(())
}
