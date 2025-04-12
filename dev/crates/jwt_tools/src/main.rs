use clap::{Args, Parser, Subcommand};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::json;
use time::OffsetDateTime;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Secutils.dev JWT tools.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a new JWT token.
    Generate(GenerateArgs),
}

#[derive(Args, Debug)]
struct GenerateArgs {
    /// Secret key used to sign JWT token.
    #[arg(short, long)]
    secret: String,
    /// JWT `sub` claim.
    #[arg(long)]
    sub: String,
    /// JWT `exp` claim.
    #[arg(long)]
    exp: humantime::Duration,
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();

    let args = Cli::parse();
    match &args.command {
        Commands::Generate(generate_args) => {
            let exp = OffsetDateTime::now_utc() + *generate_args.exp.as_ref();
            let jwt = encode(
                &Header::default(),
                &json!({
                    "sub": generate_args.sub,
                    "exp": exp.unix_timestamp(),
                }),
                &EncodingKey::from_secret(generate_args.secret.as_bytes()),
            )?;
            println!("{jwt}");
        }
    }

    Ok(())
}
