use reqwest::Client;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use indicatif::{ProgressBar, ProgressStyle};
use std::error::Error;
use std::env;
use futures_util::StreamExt;

async fn download_file(client: &Client, url: &str, file_path: &Path) -> Result<(), Box<dyn Error>> {
    let response = client.get(url).send().await?;
    let total_size = response
        .content_length()
        .ok_or("Failed to get content length")?;
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{bar:40.cyan/blue} {bytes}/{total_bytes} ({eta})")
            .progress_chars("##-"),
    );
    let mut file = File::create(file_path)?;
    let mut downloaded = 0;
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }
    pb.finish_with_message("Download complete");
    println!("File downloaded successfully to {:?}", file_path);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <url>", args[0]);
        std::process::exit(1);
    }

    let url = &args[1];
    let file_name = url.split('/').last().unwrap_or("downloaded_file");

    let client = Client::builder()
        .pool_max_idle_per_host(10)
        .build()?;

    let file_path = Path::new(file_name);
    download_file(&client, url, file_path).await?;

    Ok(())
}