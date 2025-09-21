use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use reqwest::Client;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use std::process::Command;

pub async fn download_with_progress(
    url: &str,
    out_path: &str,
    user_agent: &str,
    referer: &str,
    resume: bool,
) -> Result<()> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_str(user_agent).unwrap());
    headers.insert(REFERER, HeaderValue::from_str(referer).unwrap());
    let client = Client::builder()
        .default_headers(headers)
        .http1_only()
        .build()?;

    use reqwest::header::{RANGE, CONTENT_RANGE};
    let path = Path::new(out_path);
    let mut existing: u64 = 0;
    if resume {
        if let Ok(meta) = tokio::fs::metadata(path).await { existing = meta.len(); }
    }
    let req = if existing > 0 { client.get(url).header(RANGE, format!("bytes={}-", existing)) } else { client.get(url) };
    let resp = req.send().await?;
    let status = resp.status();
    if !(status.is_success() || status.as_u16() == 206) {
        return Err(anyhow!("download status {}", status));
    }
    let total = match (status.as_u16(), resp.headers().get(CONTENT_RANGE)) {
        (206, Some(cr)) => {
            // format: bytes START-END/TOTAL
            let s = cr.to_str().unwrap_or("");
            s.rsplit('/').next().and_then(|t| t.parse::<u64>().ok()).unwrap_or(0)
        }
        _ => resp.content_length().unwrap_or(0),
    };
    let pb = if total > 0 {
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::with_template(
                "{bar:40.cyan/blue} {bytes}/{total_bytes} ({bytes_per_sec}) ETA {eta}",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        Some(pb)
    } else {
        None
    };

    if let Some(parent) = path.parent() { tokio::fs::create_dir_all(parent).await.ok(); }
    let mut file = if existing > 0 && status.as_u16() == 206 {
        let f = tokio::fs::OpenOptions::new().append(true).open(path).await?;
        if let Some(ref pb) = pb { pb.set_position(existing); }
        f
    } else {
        File::create(path).await?
    };
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await.transpose().context("read chunk")? {
        file.write_all(&chunk).await?;
        if let Some(ref pb) = pb { pb.inc(chunk.len() as u64); }
    }
    file.flush().await?;
    if let Some(pb) = pb { pb.finish_with_message("done"); }
    Ok(())
}

pub async fn ffmpeg_mux(video_path: &str, audio_path: &str, out_path: &str) -> Result<()> {
    // Check ffmpeg presence
    let status = Command::new("ffmpeg")
        .arg("-version")
        .status()
        .context("invoke ffmpeg")?;
    if !status.success() {
        return Err(anyhow!("ffmpeg not available"));
    }

    let status = Command::new("ffmpeg")
        .args(["-y", "-i", video_path, "-i", audio_path, "-c", "copy", out_path])
        .status()
        .context("ffmpeg mux run")?;
    if !status.success() {
        return Err(anyhow!("ffmpeg mux failed with status {:?}", status.code()));
    }
    Ok(())
}

// Needed for bytes_stream() iteration
use futures_util::StreamExt;
