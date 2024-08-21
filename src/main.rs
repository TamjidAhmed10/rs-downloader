use reqwest::Client;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::error::Error;
use std::env;
use futures_util::StreamExt;
use tokio::task;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time;
use tokio::sync::Mutex;
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
    cursor::MoveTo,
};
use std::io::stdout;

#[derive(Debug)]
enum DownloadError {
    ReqwestError(reqwest::Error),
    IoError(std::io::Error),
    Other(String),
}

impl std::error::Error for DownloadError {}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::ReqwestError(e) => write!(f, "Reqwest error: {}", e),
            DownloadError::IoError(e) => write!(f, "IO error: {}", e),
            DownloadError::Other(s) => write!(f, "Other error: {}", s),
        }
    }
}

impl From<reqwest::Error> for DownloadError {
    fn from(err: reqwest::Error) -> Self {
        DownloadError::ReqwestError(err)
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(err: std::io::Error) -> Self {
        DownloadError::IoError(err)
    }
}

struct DownloadStats {
    total_bytes: u64,
    total_size: u64,
    start_time: Instant,
}

async fn download_file(client: &Client, url: &str, file_path: &Path, stats: Arc<Mutex<DownloadStats>>) -> Result<(), DownloadError> {
    let response = client.get(url).send().await?;
    let total_size = response.content_length().unwrap_or(0);

    {
        let mut stats = stats.lock().await;
        stats.total_size += total_size;
    }

    let mut file = File::create(file_path)?;
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        
        let mut stats = stats.lock().await;
        stats.total_bytes += chunk.len() as u64;
    }

    Ok(())
}

async fn update_progress_and_speed(stats: Arc<Mutex<DownloadStats>>) {
    loop {
        time::sleep(Duration::from_millis(500)).await;
        let stats = stats.lock().await;
        let elapsed = stats.start_time.elapsed().as_secs_f64();
        let speed = (stats.total_bytes as f64) / elapsed / 1_000_000.0; // MB/s
        
        let progress = if stats.total_size > 0 {
            (stats.total_bytes as f64 / stats.total_size as f64) * 100.0
        } else {
            0.0
        };
        
        execute!(
            stdout(),
            MoveTo(0, 0),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Green),
            Print(format!("Total progress: {:.2}%", progress)),
            ResetColor,
            MoveTo(0, 1),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Blue),
            Print(format!("Current download speed: {:.2} MB/s", speed)),
            ResetColor
        ).unwrap();
        
        stdout().flush().unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <url1> [url2] [url3] ...", args[0]);
        std::process::exit(1);
    }

    let client = Client::builder()
        .pool_max_idle_per_host(10)
        .build()?;

    println!("Maximum idle connections per host: 10");

    let stats = Arc::new(Mutex::new(DownloadStats {
        total_bytes: 0,
        total_size: 0,
        start_time: Instant::now(),
    }));

    let progress_stats = stats.clone();
    let progress_handle = task::spawn(async move {
        update_progress_and_speed(progress_stats).await;
    });

    let mut handles = vec![];

    for url in args.into_iter().skip(1) {
        let file_name = url.split('/').last().unwrap_or("downloaded_file").to_string();
        let file_path = Path::new(&file_name).to_path_buf();

        let client = client.clone();
        let stats = stats.clone();
        
        let handle = task::spawn(async move {
            download_file(&client, &url, &file_path, stats).await
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    // Stop the progress update task
    progress_handle.abort();

    execute!(
        stdout(),
        MoveTo(0, 3),
        Clear(ClearType::FromCursorDown)
    )?;

    println!("All downloads completed.");

    Ok(())
}