use futures_util::stream::StreamExt;
use indicatif::{ ProgressBar, ProgressStyle };
use reqwest::{ Client, header };
use serde_json::Value;
use std::{ process::Command, time::{ Duration, Instant } };
use tokio::{ fs::{ File, remove_file }, io::AsyncWriteExt };
use futures_util::Stream;
use std::pin::Pin;
use bytes::Bytes;
use thiserror::Error;

#[derive(Debug)]
pub struct YouTubeDownloader {
    client: Client,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP client error: {0}")]
    Client(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("API error: {0}")]
    Api(String),

    #[error("Conversion error: {0}")]
    Conversion(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl YouTubeDownloader {
    /// Creates a new YouTubeDownloader instance with default configuration
    ///
    /// # Example
    /// ```
    /// use rustytdown::YouTubeDownloader;
    ///
    /// let downloader = YouTubeDownloader::new().unwrap();
    /// ```
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(Error::Client)?;
        Ok(Self { client })
    }

    /// Gets the direct video URL for a YouTube video ID
    ///
    /// # Arguments
    /// * `video_id` - The YouTube video ID (e.g. "dQw4w9WgXcQ")
    ///
    /// # Example
    /// ```
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use rustytdown::YouTubeDownloader;
    ///
    /// let downloader = YouTubeDownloader::new()?;
    /// let url = downloader.get_video_url("dQw4w9WgXcQ").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_video_url(&self, video_id: &str) -> Result<String> {
        let info_url = format!(
            "https://www.youtube.com/youtubei/v1/player?key=AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vz_yYM39w&prettyPrint=false"
        );

        let json_data =
            serde_json::json!({
            "videoId": video_id,
            "context": {
                "client": {
                    "hl": "en",
                    "gl": "US", 
                    "clientName": "ANDROID",
                    "clientVersion": "18.11.34",
                    "androidSdkVersion": 31,
                    "userAgent": "com.google.android.youtube/18.11.34 (Linux; U; Android 12)",
                    "platform": "MOBILE"
                }
            },
            "playbackContext": {
                "contentPlaybackContext": {
                    "html5Preference": "HTML5_PREF_WANTS"
                }
            },
            "racyCheckOk": true,
            "contentCheckOk": true
        });

        let response = self.client
            .post(&info_url)
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::USER_AGENT,
                "com.google.android.youtube/18.11.34 (Linux; U; Android 12)"
            )
            .header("X-YouTube-Client-Name", "3")
            .header("X-YouTube-Client-Version", "18.11.34")
            .json(&json_data)
            .send().await?;

        if !response.status().is_success() {
            return Err(
                Error::Api(
                    format!(
                        "API request failed with status: {} - Body: {}",
                        response.status(),
                        response.text().await?
                    )
                )
            );
        }

        let json: Value = response.json().await?;

        let streaming_data = json
            .get("streamingData")
            .ok_or_else(|| Error::Api("No streamingData found in response".into()))?;

        let formats = streaming_data["formats"]
            .as_array()
            .or_else(|| streaming_data["adaptiveFormats"].as_array())
            .ok_or_else(|| Error::Api("No formats or adaptiveFormats found".into()))?;

        let video_url = formats
            .iter()
            .filter_map(|format| format["url"].as_str())
            .next()
            .ok_or_else(|| Error::Api("No valid URL found".into()))?
            .to_string();

        Ok(video_url)
    }

    /// Downloads a YouTube video and converts it to FLAC audio format
    ///
    /// # Arguments
    /// * `video_id` - The YouTube video ID (e.g. "dQw4w9WgXcQ")
    ///
    /// # Example
    /// ```
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use rustytdown::YouTubeDownloader;
    ///
    /// let downloader = YouTubeDownloader::new()?;
    /// let audio_path = downloader.download_and_convert("dQw4w9WgXcQ").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_and_convert(&self, video_id: &str) -> Result<String> {
        let start_time = Instant::now();
        let video_path = format!("{video_id}.mp4");
        let audio_path = format!("{video_id}.flac");

        let url = self.get_video_url(video_id).await?;

        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
            )
                .map_err(|e| Error::Api(e.to_string()))?
                .progress_chars("#>-")
        );

        let ttfb_start = Instant::now();
        let res = self.client
            .get(&url)
            .header(
                header::USER_AGENT,
                "com.google.android.youtube/18.11.34 (Linux; U; Android 12)"
            )
            .send().await?;

        let ttfb = ttfb_start.elapsed();
        println!("Time to First Byte: {:.2?}", ttfb);

        let total_size = res.content_length().unwrap_or(0);
        pb.set_length(total_size);

        let mut file = File::create(&video_path).await?;
        let mut stream = res.bytes_stream();
        let mut downloaded = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
            file.write_all(&chunk).await?;
        }

        pb.finish_with_message("Converting to audio...");

        let status = Command::new("ffmpeg")
            .args([
                "-i",
                &video_path,
                "-vn",
                "-acodec",
                "flac",
                "-compression_level",
                "8",
                "-y",
                &audio_path,
            ])
            .status()?;

        if !status.success() {
            return Err(Error::Conversion("Failed to convert video to audio".into()));
        }

        remove_file(&video_path).await?;

        let total_duration = start_time.elapsed();
        println!(
            "Download and conversion complete! TTFB: {:.2?}, Total time: {:.2?}",
            ttfb,
            total_duration
        );

        Ok(audio_path)
    }

    /// Downloads a YouTube video and saves it as an MP4 file
    ///
    /// # Arguments
    /// * `video_id` - The YouTube video ID (e.g. "dQw4w9WgXcQ")
    ///
    /// # Example
    /// ```
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use rustytdown::YouTubeDownloader;
    ///
    /// let downloader = YouTubeDownloader::new()?;
    /// let video_path = downloader.download_video("dQw4w9WgXcQ").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_video(&self, video_id: &str) -> Result<String> {
        let start_time = Instant::now();
        let video_path = format!("{video_id}.mp4");
        let url = self.get_video_url(video_id).await?;

        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
            )
                .map_err(|e| Error::Api(e.to_string()))?
                .progress_chars("#>-")
        );

        let ttfb_start = Instant::now();
        let res = self.client
            .get(&url)
            .header(
                header::USER_AGENT,
                "com.google.android.youtube/18.11.34 (Linux; U; Android 12)"
            )
            .send().await?;

        let ttfb = ttfb_start.elapsed();
        println!("Time to First Byte: {:.2?}", ttfb);

        let total_size = res.content_length().unwrap_or(0);
        pb.set_length(total_size);

        let mut file = File::create(&video_path).await?;
        let mut stream = res.bytes_stream();
        let mut downloaded = 0u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
            file.write_all(&chunk).await?;
        }

        let total_duration = start_time.elapsed();
        println!("Download complete! TTFB: {:.2?}, Total time: {:.2?}", ttfb, total_duration);

        Ok(video_path)
    }

    /// Streams a YouTube video as bytes
    ///
    /// # Arguments
    /// * `video_id` - The YouTube video ID (e.g. "dQw4w9WgXcQ")
    ///
    /// # Example
    /// ```
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use rustytdown::YouTubeDownloader;
    /// use futures_util::StreamExt;
    ///
    /// let downloader = YouTubeDownloader::new()?;
    /// let (mut stream, size) = downloader.stream_video("dQw4w9WgXcQ").await?;
    ///
    /// while let Some(chunk) = stream.next().await {
    ///     let bytes = chunk?;
    ///     // Process bytes...
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stream_video(
        &self,
        video_id: &str
    ) -> Result<(Pin<Box<dyn Stream<Item = std::result::Result<Bytes, Error>> + Send>>, u64)> {
        let url = self.get_video_url(video_id).await?;

        let res = self.client
            .get(&url)
            .header(
                header::USER_AGENT,
                "com.google.android.youtube/18.11.34 (Linux; U; Android 12)"
            )
            .send().await?;

        let content_length = res.content_length().unwrap_or(0);
        let stream = res.bytes_stream().map(|item| item.map_err(Error::Client));
        Ok((Box::pin(stream), content_length))
    }
}
