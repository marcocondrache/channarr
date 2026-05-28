mod app;
mod database;
pub mod m3u;
mod server;
mod state;
mod telemetry;

use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(long, env = "BIND", default_value = "0.0.0.0:8080")]
    bind: SocketAddr,

    #[arg(long, env = "RUST_LOG", default_value = "info")]
    log: String,

    #[arg(long, env = "DATA_DIR", default_value = "./data")]
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::try_parse()?;
    let _telemetry = telemetry::Telemetry::init(&args.log)?;
    let db = database::connect(&args.data_dir).await?;
    let state = state::AppState::new(db);

    server::serve(app::router(state), args.bind).await
}
