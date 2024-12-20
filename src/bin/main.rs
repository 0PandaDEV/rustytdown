use rustytdown::YouTubeDownloader;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    let downloader = YouTubeDownloader::new()?;
    downloader.download_and_convert("dQw4w9WgXcQ").await?;
    
    Ok(())
}
