use rustytdown::YouTubeDownloader;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let video_id = "dQw4w9WgXcQ";
    println!("Starting download test for video {}", video_id);
    
    let downloader = YouTubeDownloader::new()?;
    let video_path = downloader.download_and_convert(video_id).await?;
    println!("Download completed successfully! File saved as: {}", video_path);
    
    Ok(())
}
