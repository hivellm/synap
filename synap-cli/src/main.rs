use anyhow::{Context, Result};
use clap::Parser;
use colored::Colorize;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde_json::json;
use std::time::Instant;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "synap-cli")]
#[command(about = "Synap CLI - Redis-like command-line interface", long_about = None)]
struct Args {
    /// Server host
    #[arg(short = 'h', long, default_value = "127.0.0.1")]
    host: String,

    /// Server port
    #[arg(short = 'p', long, default_value = "15500")]
    port: u16,

    /// Command to execute (if not in interactive mode)
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

struct SynapClient {
    base_url: String,
    client: reqwest::Client,
}

impl SynapClient {
    fn new(host: &str, port: u16) -> Self {
        Self {
            base_url: format!("http://{}:{}", host, port),
            client: reqwest::Client::new(),
        }
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
            "HELP" => self.help_text()?,
            _ => return Err(anyhow::anyhow!("Unknown command: {}", command)),
        };

        let elapsed = start.elapsed();
        Ok(format!(
            "{}\n{}",
            response,
            format!("({:.2?})", elapsed).dimmed()
        ))
    }

    async fn cmd_set(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Err(anyhow::anyhow!("Usage: SET key value [ttl]"));
        }

        let ttl = args.get(2).and_then(|s| s.parse::<u64>().ok());

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.set",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "key": args[0],
                    "value": args[1],
                    "ttl": ttl
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            Ok("OK".green().to_string())
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_get(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: GET key"));
        }

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.get",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "key": args[0]
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let payload = &res["payload"];
            if payload["found"].as_bool().unwrap_or(false) {
                let value = &payload["value"];
                Ok(format!(
                    "\"{}\"",
                    value.as_str().unwrap_or(&value.to_string())
                ))
            } else {
                Ok("(nil)".dimmed().to_string())
            }
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_del(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: DEL key [key ...]"));
        }

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.mdel",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "keys": args
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let deleted = res["payload"]["deleted"].as_u64().unwrap_or(0);
            Ok(format!("(integer) {}", deleted))
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_exists(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: EXISTS key"));
        }

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.exists",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "key": args[0]
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let exists = res["payload"]["exists"].as_bool().unwrap_or(false);
            Ok(format!("(integer) {}", if exists { 1 } else { 0 }))
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_incr(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: INCR key [amount]"));
        }

        let amount = args.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.incr",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "key": args[0],
                    "amount": amount
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let value = res["payload"]["value"].as_i64().unwrap_or(0);
            Ok(format!("(integer) {}", value))
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_decr(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: DECR key [amount]"));
        }

        let amount = args.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(1);

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.decr",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "key": args[0],
                    "amount": amount
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let value = res["payload"]["value"].as_i64().unwrap_or(0);
            Ok(format!("(integer) {}", value))
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_expire(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 {
            return Err(anyhow::anyhow!("Usage: EXPIRE key seconds"));
        }

        let ttl = args[1]
            .parse::<u64>()
            .context("TTL must be a valid number")?;

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.expire",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "key": args[0],
                    "ttl": ttl
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let result = res["payload"]["result"].as_bool().unwrap_or(false);
            Ok(format!("(integer) {}", if result { 1 } else { 0 }))
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_ttl(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: TTL key"));
        }

        let res = self
            .client
            .get(format!("{}/kv/get/{}", self.base_url, args[0]))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["found"].as_bool().unwrap_or(false) {
            if let Some(ttl) = res["ttl"].as_u64() {
                Ok(format!("(integer) {}", ttl))
            } else {
                Ok("(integer) -1".to_string()) // No expiration
            }
        } else {
            Ok("(integer) -2".to_string()) // Key doesn't exist
        }
    }

    async fn cmd_persist(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: PERSIST key"));
        }

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.persist",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "key": args[0]
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let result = res["payload"]["result"].as_bool().unwrap_or(false);
            Ok(format!("(integer) {}", if result { 1 } else { 0 }))
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_keys(&self, args: &[String]) -> Result<String> {
        let pattern = args.first().map(|s| s.as_str());

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.scan",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "prefix": pattern,
                    "limit": 1000
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let empty_vec = vec![];
            let keys = res["payload"]["keys"].as_array().unwrap_or(&empty_vec);
            let output = keys
                .iter()
                .enumerate()
                .map(|(i, k)| format!("{}) \"{}\"", i + 1, k.as_str().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("\n");
            Ok(output)
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_scan(&self, args: &[String]) -> Result<String> {
        self.cmd_keys(args).await
    }

    async fn cmd_dbsize(&self) -> Result<String> {
        let res = self
            .client
            .get(format!("{}/kv/stats", self.base_url))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        let total = res["total_keys"].as_u64().unwrap_or(0);
        Ok(format!("(integer) {}", total))
    }

    async fn cmd_flushdb(&self) -> Result<String> {
        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.flushdb",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {}
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            Ok("OK".green().to_string())
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_flushall(&self) -> Result<String> {
        self.cmd_flushdb().await
    }

    async fn cmd_stats(&self) -> Result<String> {
        let res = self
            .client
            .get(format!("{}/kv/stats", self.base_url))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

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
        let res = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["status"] == "healthy" {
            Ok("PONG".green().to_string())
        } else {
            Ok("Server unhealthy".red().to_string())
        }
    }

    async fn cmd_mset(&self, args: &[String]) -> Result<String> {
        if args.len() < 2 || args.len() % 2 != 0 {
            return Err(anyhow::anyhow!("Usage: MSET key value [key value ...]"));
        }

        let mut pairs = Vec::new();
        for chunk in args.chunks(2) {
            pairs.push(json!({
                "key": chunk[0],
                "value": chunk[1]
            }));
        }

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.mset",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "pairs": pairs
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            Ok("OK".green().to_string())
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    async fn cmd_mget(&self, args: &[String]) -> Result<String> {
        if args.is_empty() {
            return Err(anyhow::anyhow!("Usage: MGET key [key ...]"));
        }

        let res = self
            .client
            .post(format!("{}/api/v1/command", self.base_url))
            .json(&json!({
                "command": "kv.mget",
                "request_id": uuid::Uuid::new_v4().to_string(),
                "payload": {
                    "keys": args
                }
            }))
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if res["success"].as_bool().unwrap_or(false) {
            let empty_vec = vec![];
            let values = res["payload"]["values"].as_array().unwrap_or(&empty_vec);
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
        } else {
            Err(anyhow::anyhow!(
                "Error: {}",
                res["error"].as_str().unwrap_or("Unknown")
            ))
        }
    }

    fn help_text(&self) -> Result<String> {
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
"#,
            "Synap CLI - Available Commands".bold().cyan(),
            "Basic Commands:".bold(),
            "TTL Commands:".bold(),
            "Key Discovery:".bold(),
            "Batch Commands:".bold(),
            "Database Commands:".bold(),
            "Server Commands:".bold(),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for CLI output
    // Use info level by default to show user-facing messages
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
    let client = SynapClient::new(&args.host, args.port);

    // Check if running in command mode or interactive mode
    if !args.command.is_empty() {
        // Command mode: execute single command and exit
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
        // Interactive mode
        run_interactive(client, &args.host, args.port).await
    }
}

async fn run_interactive(client: SynapClient, host: &str, port: u16) -> Result<()> {
    info!(
        "{}",
        format!("Synap CLI v{}", env!("CARGO_PKG_VERSION"))
            .bold()
            .cyan()
    );
    info!("Connected to {}:{}", host, port);
    info!("Type {} for available commands\n", "HELP".bold());

    let mut rl = DefaultEditor::new()?;

    loop {
        let prompt = format!("{}> ", format!("synap {}:{}", host, port).green());
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
