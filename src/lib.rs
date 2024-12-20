use futures_util::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{Client, header};
use serde_json::Value;
use std::{process::Command, time::{Duration, Instant}};
use tokio::{fs::{File, remove_file}, io::AsyncWriteExt};
use futures_util::Stream;
use std::pin::Pin;
use bytes::Bytes;

pub struct YouTubeDownloader {
    client: Client,
}

impl YouTubeDownloader {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self { client })
    }

    pub async fn get_video_url(&self, video_id: &str) -> Result<String, Box<dyn std::error::Error>> {
        let info_url = format!(
            "https://www.youtube.com/youtubei/v1/player?key=AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vz_yYM39w&prettyPrint=false"
        );

        let json_data = serde_json::json!({
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
                "com.google.android.youtube/18.11.34 (Linux; U; Android 12)",
            )
            .header("X-YouTube-Client-Name", "3")
            .header("X-YouTube-Client-Version", "18.11.34")
            .json(&json_data)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!(
                "API request failed with status: {} - Body: {}",
                response.status(),
                response.text().await?
            )
            .into());
        }

        let json: Value = response.json().await?;

        let streaming_data = json
            .get("streamingData")
            .ok_or("No streamingData found in response")?;

        let formats = streaming_data["formats"]
            .as_array()
            .or_else(|| streaming_data["adaptiveFormats"].as_array())
            .ok_or("No formats or adaptiveFormats found")?;

        println!("\nAvailable formats:");
        for (i, format) in formats.iter().enumerate() {
            let quality = format["quality"].as_str().unwrap_or("unknown");
            let mime_type = format["mimeType"].as_str().unwrap_or("unknown");
            let bitrate = format["bitrate"].as_u64().unwrap_or(0) / 1000;
            println!("{}. Quality: {}, Type: {}, Bitrate: {}kbps", i + 1, quality, mime_type, bitrate);
        }

        let video_url = formats
            .iter()
            .filter_map(|format| {
                let url = format["url"].as_str();
                url
            })
            .next()
            .ok_or("No valid URL found")?
            .to_string();

        Ok(video_url)
    }

    pub async fn download_and_convert(&self, video_id: &str) -> Result<String, Box<dyn std::error::Error>> {
        let start_time = Instant::now();

        let video_path = format!("{video_id}.mp4");
        let audio_path = format!("{video_id}.flac");

        let url = self.get_video_url(video_id).await?;

        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
            )?
            .progress_chars("#>-")
        );

        let ttfb_start = Instant::now();
        let res = self.client
            .get(&url)
            .header(
                header::USER_AGENT,
                "com.google.android.youtube/18.11.34 (Linux; U; Android 12)",
            )
            .send()
            .await?;

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
                "-i", &video_path,
                "-vn",
                "-acodec", "flac",
                "-compression_level", "8",
                &audio_path
            ])
            .status()?;

        if !status.success() {
            return Err("Failed to convert video to audio".into());
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

    pub async fn download_video(&self, video_id: &str) -> Result<String, Box<dyn std::error::Error>> {
        let start_time = Instant::now();

        let video_path = format!("{video_id}.mp4");
        let url = self.get_video_url(video_id).await?;

        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
            )?
            .progress_chars("#>-")
        );

        let ttfb_start = Instant::now();
        let res = self.client
            .get(&url)
            .header(
                header::USER_AGENT,
                "com.google.android.youtube/18.11.34 (Linux; U; Android 12)",
            )
            .send()
            .await?;

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
        println!(
            "Download complete! TTFB: {:.2?}, Total time: {:.2?}",
            ttfb,
            total_duration
        );

        Ok(video_path)
    }

    pub async fn stream_video(&self, video_id: &str) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>, Box<dyn std::error::Error>> {
        let url = self.get_video_url(video_id).await?;

        let res = self.client
            .get(&url)
            .header(
                header::USER_AGENT,
                "com.google.android.youtube/18.11.34 (Linux; U; Android 12)",
            )
            .send()
            .await?;

        Ok(Box::pin(res.bytes_stream()))
    }
}
