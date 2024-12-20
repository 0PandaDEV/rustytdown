# RustyTDown

A lightweight and efficient YouTube video downloader written in Rust. This tool allows you to download YouTube videos with just a few essential dependencies for core functionality!

## Features

- 🚀 High-performance async downloads using reqwest
- 🎯 Carefully selected minimal dependencies for core functionality
- 📈 Download statistics and TTFB measurements  
- 🔄 Streaming support
- 🪶 Lightweight and efficient

## Prerequisites

- Rust 1.75 or higher
- FFmpeg (required for audio conversion)

## Installation

### From crates.io

```bash
cargo install rustytdown
```

### From source

```bash
git clone https://github.com/0pandadev/rustytdown
cd rustytdown
cargo install --path .
```

## Usage

```toml
[dependencies]
rustytdown = "0.1.0"
```

Example code:

```rust
use rustytdown::YouTubeDownloader;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    let downloader = YouTubeDownloader::new()?;
    downloader.download_and_convert("dQw4w9WgXcQ").await?;
    
    Ok(())
}
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
