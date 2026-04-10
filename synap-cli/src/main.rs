use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde_json::{Value, json};
use std::time::Instant;
use synap_sdk::{SynapClient, SynapConfig};
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "synap-cli")]
#[command(
    about = "Synap CLI - Redis-like command-line interface for Synap server",
    long_about = None
)]
struct Args {
    /// Server URL with protocol auto-detection.
    ///   http://host:15500   — HTTP/REST (default)
    ///   synap://host:15501  — SynapRPC binary protocol
    ///   resp3://host:6379   — RESP3 Redis-compatible protocol
    #[arg(short = 'u', long)]
    url: Option<String>,

    /// Server host (used when --url is not set)
    #[arg(short = 'H', long, default_value = "127.0.0.1")]
    host: String,

    /// Server port (used when --url is not set)
    #[arg(short = 'p', long, default_value = "15500")]
    port: u16,

    /// Transport protocol when using -h/-p: http, rpc, resp3
    #[arg(long, default_value = "http")]
    transport: String,

    /// Command to execute (non-interactive mode)
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

impl Args {
    fn effective_url(&self) -> String {
        if let Some(ref url) = self.url {
            return url.clone();
        }
        match self.transport.to_lowercase().as_str() {
            "rpc" | "synap" | "synaprpc" => format!("synap://{}:{}", self.host, self.port),
            "resp3" | "redis" => format!("resp3://{}:{}", self.host, self.port),
            _ => format!("http://{}:{}", self.host, self.port),
        }
    }
}

// ── CLI client wrapping the SDK ──────────────────────────────────────────────

struct CliClient {
    sdk: SynapClient,
}

impl CliClient {
    fn new(url: &str) -> Result<Self> {
        let config = SynapConfig::new(url);
        let sdk =
            SynapClient::new(config).map_err(|e| anyhow::anyhow!("Failed to connect: {}", e))?;
        Ok(Self { sdk })
    }

    async fn execute_command(&self, command: &str, args: &[String]) -> Result<String> {
        let start = Instant::now();

        let response = match command.to_uppercase().as_str() {
            "SET" => self.cmd_set(args).await?,
            "GET" => self.cmd_get(args).await?,
            "DEL" | "DELETE" => self.cmd_del(args).await?,
            "EXISTS" => self.cmd_exists(args).await?,
            "INCR" => self.cmd_incr(args).await?,
            "DECR" => self.cmd_decr(args).await?,
            "EXPIRE" => self.cmd_expire(args).await?,
            "TTL" => self.cmd_ttl(args).await?,
            "PERSIST" => self.cmd_persist(args).await?,
            "KEYS" => self.cmd_keys(args).await?,
            "SCAN" => self.cmd_scan(args).await?,
            "DBSIZE" => self.cmd_dbsize().await?,
            "FLUSHDB" => self.cmd_flushdb().await?,
            "FLUSHALL" => self.cmd_flushall().await?,
            "INFO" | "STATS" => self.cmd_stats().await?,
            "PING" => self.cmd_ping().await?,
            "MSET" => self.cmd_mset(args).await?,
            "MGET" => self.cmd_mget(args).await?,
            "HELP" => Self::help_text()?,
            _ => return Err(anyhow::anyhow!("Unknown command: {}", command)),
        };

        let elapsed = start.elapsed();
        Ok(format!(
            "{}\n{}",
            response,
            format!("({:.2?})", elapsed).dimmed()
        ))
    }

    // ── Command helpers ──────────────────────────────────────────────────────

    async fn send(&self, cmd: &str, payload: Value) -> Result<Value> {
        self.sdk
            .send_command(cmd, payload)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    async fn cmd_set(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Err(anyhow::anyhow!("Usage: SET key value [ttl]"));
        }
        let ttl = args.get(2).and_then(|s| s.parse::<u64>().ok());
        self.send(
            "kv.set",
            json!({"key": args[0], "value": args[1], "ttl": ttl}),
        )
        .await?;
        Ok("OK".green().to_string())
    }

    async fn cmd_get(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: GET key"));
        }
        let res = self.send("kv.get", json!({"key": args[0]})).await?;
        if res.is_null() {
            Ok("(nil)".dimmed().to_string())
        } else if let Some(obj) = res.as_object() {
            // HTTP returns {"found": bool, "value": ...}
            if obj.get("found").and_then(|v| v.as_bool()).unwrap_or(false) {
                let val = &obj["value"];
                Ok(format!("\"{}\"", val.as_str().unwrap_or(&val.to_string())))
            } else {
                Ok("(nil)".dimmed().to_string())
            }
        } else {
            // Binary transports return the value directly
            Ok(format!("\"{}\"", res.as_str().unwrap_or(&res.to_string())))
        }
    }

    async fn cmd_del(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: DEL key [key ...]"));
        }
        let res = self.send("kv.mdel", json!({"keys": args})).await?;
        let deleted = res["deleted"].as_u64().unwrap_or(0);
        Ok(format!("(integer) {}", deleted))
    }

    async fn cmd_exists(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: EXISTS key"));
        }
        let res = self.send("kv.exists", json!({"key": args[0]})).await?;
        let exists = res["exists"].as_bool().unwrap_or(false);
        Ok(format!("(integer) {}", if exists { 1 } else { 0 }))
    }

    async fn cmd_incr(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: INCR key [amount]"));
        }
        let amount = args.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
        let res = self
            .send("kv.incr", json!({"key": args[0], "amount": amount}))
            .await?;
        let value = res["value"].as_i64().or(res.as_i64()).unwrap_or(0);
        Ok(format!("(integer) {}", value))
    }

    async fn cmd_decr(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: DECR key [amount]"));
        }
        let amount = args.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);
        let res = self
            .send("kv.decr", json!({"key": args[0], "amount": amount}))
            .await?;
        let value = res["value"].as_i64().or(res.as_i64()).unwrap_or(0);
        Ok(format!("(integer) {}", value))
    }

    async fn cmd_expire(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Err(anyhow::anyhow!("Usage: EXPIRE key seconds"));
        }
        let ttl = args[1]
            .parse::<u64>()
            .context("TTL must be a valid number")?;
        let res = self
            .send("kv.expire", json!({"key": args[0], "ttl": ttl}))
            .await?;
        let result = res["result"].as_bool().unwrap_or(false);
        Ok(format!("(integer) {}", if result { 1 } else { 0 }))
    }

    async fn cmd_ttl(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: TTL key"));
        }
        let res = self.send("kv.ttl", json!({"key": args[0]})).await?;
        let ttl = res["ttl"].as_i64().or(res.as_i64()).unwrap_or(-2);
        Ok(format!("(integer) {}", ttl))
    }

    async fn cmd_persist(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: PERSIST key"));
        }
        let res = self.send("kv.persist", json!({"key": args[0]})).await?;
        let result = res["result"].as_bool().unwrap_or(false);
        Ok(format!("(integer) {}", if result { 1 } else { 0 }))
    }

    async fn cmd_keys(&self, args: &[String]) -> Result<String> {
        let pattern = args.first().map(|s| s.as_str());
        let res = self
            .send("kv.scan", json!({"prefix": pattern, "limit": 1000}))
            .await?;
        let empty = vec![];
        let keys = res["keys"].as_array().unwrap_or(&empty);
        if keys.is_empty() {
            return Ok("(empty list)".dimmed().to_string());
        }
        let output = keys
            .iter()
            .enumerate()
            .map(|(i, k)| format!("{}) \"{}\"", i + 1, k.as_str().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(output)
    }

    async fn cmd_scan(&self, args: &[String]) -> Result<String> {
        self.cmd_keys(args).await
    }

    async fn cmd_dbsize(&self) -> Result<String> {
        let res = self.send("kv.dbsize", json!({})).await?;
        let size = res["size"]
            .as_u64()
            .or(res["total_keys"].as_u64())
            .or(res.as_u64())
            .unwrap_or(0);
        Ok(format!("(integer) {}", size))
    }

    async fn cmd_flushdb(&self) -> Result<String> {
        self.send("kv.flushdb", json!({})).await?;
        Ok("OK".green().to_string())
    }

    async fn cmd_flushall(&self) -> Result<String> {
        self.send("kv.flushall", json!({})).await?;
        Ok("OK".green().to_string())
    }

    async fn cmd_stats(&self) -> Result<String> {
        let res = self.send("kv.stats", json!({})).await?;
        let mut output = Vec::new();
        output.push("# Keyspace".to_string());
        output.push(format!("keys: {}", res["total_keys"]));
        output.push(format!("memory: {} bytes", res["total_memory_bytes"]));
        output.push(String::new());
        output.push("# Operations".to_string());
        output.push(format!("gets: {}", res["operations"]["gets"]));
        output.push(format!("sets: {}", res["operations"]["sets"]));
        output.push(format!("dels: {}", res["operations"]["dels"]));
        output.push(format!("hits: {}", res["operations"]["hits"]));
        output.push(format!("misses: {}", res["operations"]["misses"]));
        output.push(format!(
            "hit_rate: {:.2}%",
            res["hit_rate"].as_f64().unwrap_or(0.0) * 100.0
        ));
        Ok(output.join("\n"))
    }

    async fn cmd_ping(&self) -> Result<String> {
        // Use kv.dbsize as a connectivity check — works on all transports.
        match self.send("kv.dbsize", json!({})).await {
            Ok(_) => Ok("PONG".green().to_string()),
            Err(_) => Ok("Server unreachable".red().to_string()),
        }
    }

    async fn cmd_mset(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 || args.len() % 2 != 0 {
            return Err(anyhow::anyhow!("Usage: MSET key value [key value ...]"));
        }
        let pairs: Vec<Value> = args
            .chunks(2)
            .map(|c| json!({"key": c[0], "value": c[1]}))
            .collect();
        self.send("kv.mset", json!({"pairs": pairs})).await?;
        Ok("OK".green().to_string())
    }

    async fn cmd_mget(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: MGET key [key ...]"));
        }
        let res = self.send("kv.mget", json!({"keys": args})).await?;
        let empty = vec![];
        let values = res["values"].as_array().unwrap_or(&empty);
        let output = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if v.is_null() {
                    format!("{}) (nil)", i + 1)
                } else {
                    format!("{}) \"{}\"", i + 1, v.as_str().unwrap_or(&v.to_string()))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(output)
    }

    fn help_text() -> Result<String> {
        Ok(format!(
            r#"{}

{}
  SET key value [ttl]        Set key to hold string value with optional TTL
  GET key                    Get the value of key
  DEL key [key ...]          Delete one or more keys
  EXISTS key                 Check if key exists
  INCR key [amount]          Increment value by amount (default 1)
  DECR key [amount]          Decrement value by amount (default 1)

{}
  EXPIRE key seconds         Set timeout on key
  TTL key                    Get remaining time to live
  PERSIST key                Remove timeout from key

{}
  KEYS [pattern]             Find all keys matching pattern
  SCAN [pattern] [count]     Scan keys with optional prefix
  DBSIZE                     Return number of keys

{}
  MSET k1 v1 [k2 v2 ...]     Set multiple keys
  MGET key [key ...]         Get values of multiple keys

{}
  FLUSHDB                    Remove all keys from database
  FLUSHALL                   Remove all keys from all databases

{}
  INFO                       Get server statistics
  STATS                      Alias for INFO
  PING                       Ping the server
  HELP                       Show this help message
  QUIT                       Exit the CLI

{}
  --url synap://host:15501   SynapRPC binary protocol
  --url resp3://host:6379    RESP3 Redis-compatible protocol
  --url http://host:15500    HTTP/REST (default)
  -p 15501 --transport rpc   Shortcut for SynapRPC
"#,
            "Synap CLI - Available Commands".bold().cyan(),
            "Basic Commands:".bold(),
            "TTL Commands:".bold(),
            "Key Discovery:".bold(),
            "Batch Commands:".bold(),
            "Database Commands:".bold(),
            "Server Commands:".bold(),
            "Transport Options:".bold(),
        ))
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    tracing_subscriber::fmt()
        .with_writer(std::io::stdout)
        .with_env_filter(tracing_subscriber::EnvFilter::new(log_level))
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .init();

    let args = Args::parse();
    let url = args.effective_url();
    let client = CliClient::new(&url)?;

    if !args.command.is_empty() {
        let cmd = &args.command[0];
        let cmd_args: Vec<String> = args.command[1..].to_vec();
        match client.execute_command(cmd, &cmd_args).await {
            Ok(output) => {
                info!("{}", output);
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "{}", format!("Error: {}", e).red());
                std::process::exit(1);
            }
        }
    } else {
        run_interactive(client, &url).await
    }
}

async fn run_interactive(client: CliClient, url: &str) -> Result<()> {
    info!(
        "{}",
        format!("Synap CLI v{}", env!("CARGO_PKG_VERSION"))
            .bold()
            .cyan()
    );
    info!("Connected to {}", url);
    info!("Type {} for available commands\n", "HELP".bold());

    let mut rl = DefaultEditor::new()?;

    loop {
        let prompt = format!("{}> ", url.green());
        let readline = rl.readline(&prompt);

        match readline {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }

                rl.add_history_entry(&line)?;

                let parts: Vec<String> = line.split_whitespace().map(String::from).collect();
                if parts.is_empty() {
                    continue;
                }

                let cmd = &parts[0];
                let args = &parts[1..];

                if cmd.to_uppercase() == "QUIT" || cmd.to_uppercase() == "EXIT" {
                    info!("Goodbye!");
                    break;
                }

                match client.execute_command(cmd, args).await {
                    Ok(output) => info!("{}", output),
                    Err(e) => {
                        error!(error = %e, "{}", format!("Error: {}", e).red());
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                info!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                info!("Goodbye!");
                break;
            }
            Err(err) => {
                error!(error = ?err, "Readline error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
