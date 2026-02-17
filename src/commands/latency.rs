use crate::config::{AliasConfig, Config, ConfigError};
use colored::Colorize;
use sqlx::postgres::PgPoolOptions;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum LatencyError {
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("No successful requests")]
    NoSuccess,
}

fn parse_host_port(raw: &str) -> Result<(String, u16), LatencyError> {
    let url = Url::parse(raw).map_err(|e| LatencyError::InvalidUrl(e.to_string()))?;

    let host = url
        .host_str()
        .ok_or_else(|| LatencyError::InvalidUrl("missing host".to_string()))?
        .to_string();

    let default_port = match url.scheme() {
        "postgres" | "postgresql" => 5432,
        "https" => 443,
        "http" => 80,
        _ => 443,
    };

    let port = url.port().unwrap_or(default_port);
    Ok((host, port))
}

fn resolve_alias(alias: &Option<String>) -> Result<(String, AliasConfig), LatencyError> {
    let config = Config::load()?;

    let alias_name = alias
        .clone()
        .or(config.default.clone())
        .ok_or(ConfigError::AliasNotFound(
            "no default configured".to_string(),
        ))?;

    let alias_config = config
        .get_alias(&alias_name)
        .cloned()
        .ok_or(ConfigError::AliasNotFound(alias_name.clone()))?;

    Ok((alias_name, alias_config))
}

fn print_stats(latencies: &[f64], count: usize) {
    if latencies.is_empty() {
        println!("{} All requests failed (0/{})", "✗".red().bold(), count);
        return;
    }

    let min = latencies.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = latencies.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;

    println!("{}", "Statistics:".bold());
    println!("  Min:     {:.2} ms", min);
    println!("  Max:     {:.2} ms", max);
    println!("  Avg:     {:.2} ms", avg);
    println!(
        "  Success: {}/{}",
        latencies.len().to_string().green(),
        count
    );
}

pub async fn run(
    alias: Option<String>,
    connect: bool,
    count: usize,
    timeout: u64,
) -> Result<(), LatencyError> {
    let (alias_name, alias_config) = resolve_alias(&alias)?;

    match alias_config {
        AliasConfig::Db { database_url, .. } => {
            if connect {
                run_db_connect(&alias_name, &database_url, count, timeout).await
            } else {
                run_db_query(&alias_name, &database_url, count, timeout).await
            }
        }
        AliasConfig::Api { url, insecure, .. } => {
            if connect {
                run_http_connect(&alias_name, &url, count, timeout).await
            } else {
                run_http_reuse(&alias_name, &url, insecure, count, timeout).await
            }
        }
    }
}

// --- DB: reuse connection, measure SELECT 1 ---

async fn run_db_query(
    alias_name: &str,
    database_url: &str,
    count: usize,
    timeout: u64,
) -> Result<(), LatencyError> {
    let (host, port) = parse_host_port(database_url)?;

    println!(
        "{} Connecting to database '{}' ({}:{})",
        "→".cyan(),
        alias_name.green().bold(),
        host.cyan(),
        port.to_string().cyan(),
    );

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(timeout))
        .connect(database_url)
        .await?;

    println!(
        "{} Connected. Measuring query latency (SELECT 1)...",
        "✓".green()
    );
    println!();

    let mut latencies = Vec::with_capacity(count);

    for i in 1..=count {
        let start = Instant::now();
        let result: Result<i32, _> = sqlx::query_scalar("SELECT 1").fetch_one(&pool).await;
        let elapsed = start.elapsed();

        match result {
            Ok(_) => {
                let ms = elapsed.as_secs_f64() * 1000.0;
                latencies.push(ms);
                println!("  {} {}/{}: {:.2} ms", "✓".green(), i, count, ms);
            }
            Err(e) => {
                println!(
                    "  {} {}/{}: {}",
                    "✗".red(),
                    i,
                    count,
                    e.to_string().dimmed()
                );
            }
        }
    }

    println!();
    print_stats(&latencies, count);

    if latencies.is_empty() {
        return Err(LatencyError::NoSuccess);
    }

    Ok(())
}

// --- DB: new TCP connection each time ---

async fn run_db_connect(
    alias_name: &str,
    database_url: &str,
    count: usize,
    timeout: u64,
) -> Result<(), LatencyError> {
    let (host, port) = parse_host_port(database_url)?;
    let addr = format!("{}:{}", host, port);
    let timeout_dur = Duration::from_secs(timeout);

    println!(
        "{} Testing connection latency to database '{}' ({}:{})",
        "→".cyan(),
        alias_name.green().bold(),
        host.cyan(),
        port.to_string().cyan(),
    );
    println!();

    let mut latencies = Vec::with_capacity(count);

    for i in 1..=count {
        let start = Instant::now();
        let result = tokio::time::timeout(timeout_dur, TcpStream::connect(&addr)).await;
        let elapsed = start.elapsed();

        match result {
            Ok(Ok(_)) => {
                let ms = elapsed.as_secs_f64() * 1000.0;
                latencies.push(ms);
                println!("  {} {}/{}: {:.2} ms", "✓".green(), i, count, ms);
            }
            Ok(Err(e)) => {
                println!(
                    "  {} {}/{}: {}",
                    "✗".red(),
                    i,
                    count,
                    e.to_string().dimmed()
                );
            }
            Err(_) => {
                println!("  {} {}/{}: {}", "✗".red(), i, count, "timeout".dimmed());
            }
        }

        if i < count {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    println!();
    print_stats(&latencies, count);

    if latencies.is_empty() {
        return Err(LatencyError::NoSuccess);
    }

    Ok(())
}

// --- HTTP: reuse connection, measure layered latency ---

fn latency_url(api_url: &str, layer: &str) -> String {
    // API URL is like https://host/api/v1 → we need https://host/api/health/latency/<layer>
    let base = api_url.trim_end_matches('/');
    let base = base.strip_suffix("/v1").unwrap_or(base);
    format!("{}/health/latency/{}", base, layer)
}

struct Layer {
    name: &'static str,
    path: &'static str,
    label: &'static str,
}

const LAYERS: &[Layer] = &[
    Layer {
        name: "proxy",
        path: "proxy",
        label: "Proxy",
    },
    Layer {
        name: "api",
        path: "api",
        label: "API",
    },
    Layer {
        name: "database",
        path: "database",
        label: "API + Database",
    },
];

async fn run_http_reuse(
    alias_name: &str,
    url: &str,
    insecure: bool,
    count: usize,
    timeout: u64,
) -> Result<(), LatencyError> {
    let (host, _) = parse_host_port(url)?;

    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .danger_accept_invalid_certs(insecure);

    // Resolve *.localhost domains to 127.0.0.1 (RFC 6761)
    if host.ends_with(".localhost") {
        let port = parse_host_port(url)?.1;
        let addr: std::net::SocketAddr = ([127, 0, 0, 1], port).into();
        builder = builder.resolve(&host, addr);
    }

    let client = builder
        .build()
        .map_err(|e| LatencyError::InvalidUrl(e.to_string()))?;

    println!(
        "{} Testing latency to API '{}' ({})",
        "→".cyan(),
        alias_name.green().bold(),
        host.cyan(),
    );

    // Warmup: establish TCP + TLS connection
    let warmup_url = latency_url(url, LAYERS[0].path);
    let start = Instant::now();
    let resp = client
        .get(&warmup_url)
        .send()
        .await
        .map_err(|e| LatencyError::InvalidUrl(e.to_string()))?;
    let _ = resp.bytes().await;
    let warmup_ms = start.elapsed().as_secs_f64() * 1000.0;

    println!("{} Connected ({:.0} ms handshake)", "✓".green(), warmup_ms);

    let mut any_success = false;

    for layer in LAYERS {
        let endpoint = latency_url(url, layer.path);

        println!();
        println!("{}:", layer.label.bold());

        let mut latencies = Vec::with_capacity(count);

        for i in 1..=count {
            let start = Instant::now();
            let result = client.get(&endpoint).send().await;
            let elapsed = start.elapsed();

            match result {
                Ok(resp) if resp.status().as_u16() == 418 => {
                    let _ = resp.bytes().await;

                    if i == 1 {
                        println!(
                            "  {} {} (not configured)",
                            "─".dimmed(),
                            layer.name.dimmed()
                        );
                    }

                    break;
                }
                Ok(resp) if resp.status().is_success() => {
                    let _ = resp.bytes().await;
                    let ms = elapsed.as_secs_f64() * 1000.0;
                    latencies.push(ms);
                    println!("  {} {}/{}: {:.2} ms", "✓".green(), i, count, ms);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let _ = resp.bytes().await;
                    println!(
                        "  {} {}/{}: {}",
                        "✗".red(),
                        i,
                        count,
                        format!("HTTP {}", status).dimmed()
                    );
                }
                Err(e) => {
                    println!(
                        "  {} {}/{}: {}",
                        "✗".red(),
                        i,
                        count,
                        e.to_string().dimmed()
                    );
                }
            }
        }

        if !latencies.is_empty() {
            any_success = true;
            print_layer_stats(&latencies);
        }
    }

    if !any_success {
        return Err(LatencyError::NoSuccess);
    }

    Ok(())
}

fn print_layer_stats(latencies: &[f64]) {
    let min = latencies.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = latencies.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;

    println!(
        "  {} min {:.2} ms / avg {:.2} ms / max {:.2} ms",
        "→".dimmed(),
        min,
        avg,
        max,
    );
}

// --- HTTP: new TCP connection each time ---

async fn run_http_connect(
    alias_name: &str,
    url: &str,
    count: usize,
    timeout: u64,
) -> Result<(), LatencyError> {
    let (host, port) = parse_host_port(url)?;
    let addr = format!("{}:{}", host, port);
    let timeout_dur = Duration::from_secs(timeout);

    println!(
        "{} Testing connection latency to API '{}' ({}:{})",
        "→".cyan(),
        alias_name.green().bold(),
        host.cyan(),
        port.to_string().cyan(),
    );
    println!();

    let mut latencies = Vec::with_capacity(count);

    for i in 1..=count {
        let start = Instant::now();
        let result = tokio::time::timeout(timeout_dur, TcpStream::connect(&addr)).await;
        let elapsed = start.elapsed();

        match result {
            Ok(Ok(_)) => {
                let ms = elapsed.as_secs_f64() * 1000.0;
                latencies.push(ms);
                println!("  {} {}/{}: {:.2} ms", "✓".green(), i, count, ms);
            }
            Ok(Err(e)) => {
                println!(
                    "  {} {}/{}: {}",
                    "✗".red(),
                    i,
                    count,
                    e.to_string().dimmed()
                );
            }
            Err(_) => {
                println!("  {} {}/{}: {}", "✗".red(), i, count, "timeout".dimmed());
            }
        }

        if i < count {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    println!();
    print_stats(&latencies, count);

    if latencies.is_empty() {
        return Err(LatencyError::NoSuccess);
    }

    Ok(())
}
