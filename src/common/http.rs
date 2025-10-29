use std::io::stdout;
use std::time::Instant;

use anyhow::anyhow;
use crossterm::{
    cursor, execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType, size as term_size},
};
use futures::StreamExt;
use reqwest::{Client, header::USER_AGENT, redirect};
use tokio::io::AsyncWriteExt;

use crate::utils::path;

pub async fn download_file(url: &str, dest: &str) -> anyhow::Result<()> {
    let filename = path::get_filename(url).ok_or(anyhow!("Failed to get filename"))?;

    let client = Client::builder()
        .redirect(redirect::Policy::limited(10))
        .user_agent("Wget/1.25.0 (linux-gnu)")
        .build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP error: {}", resp.status());
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded = 0u64;
    let start = Instant::now();

    let draw_bar = |filename: &str, downloaded: u64, total: u64, start: Instant, complete: bool| {
        let width = 30;
        let percent = if total > 0 {
            downloaded as f64 / total as f64
        } else {
            0.0
        };
        let filled = (percent * width as f64).round() as usize;

        // 完了時は '>' を消して全部 '#'
        let bar = if complete {
            format!("[{}]", "#".repeat(width))
        } else {
            format!("[{}>{}]", "#".repeat(filled), " ".repeat(width - filled))
        };

        let elapsed = start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            (downloaded as f64 / elapsed) / 1_000_000.0 // MB/s
        } else {
            0.0
        };
        let speed_str = format!("{:.2} MB/s", speed);
        let time_str = format!(
            "{:02}:{:02}",
            (elapsed / 60.0) as u64,
            (elapsed % 60.0) as u64
        );
        let percent_str = format!("{:>5.1}%", percent * 100.0);

        let size_str = if total > 0 {
            format!("{}/{}", downloaded, total)
        } else {
            format!("{}", downloaded)
        };

        let left_part = format!(" {}", filename);
        let right_part = format!(
            "{}  {}  {}  {}  {}",
            size_str, speed_str, time_str, bar, percent_str
        );

        let term_width = term_size().map(|(w, _)| w).unwrap_or(80) as usize;
        let total_len = left_part.chars().count() + 1 + right_part.chars().count();
        let spaces = if term_width > total_len {
            term_width - total_len
        } else {
            1
        };
        let space_str = " ".repeat(spaces);

        let line = format!("{}{}{}", left_part, space_str, right_part);

        execute!(
            stdout(),
            cursor::MoveToColumn(0),
            Clear(ClearType::CurrentLine),
            SetForegroundColor(Color::Cyan),
            Print(line),
            ResetColor
        )
        .unwrap();
    };

    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded += chunk.len() as u64;
        file.write_all(&chunk).await?;
        draw_bar(&filename, downloaded, total, start, false);
    }

    // 完了後に ">" を消して再描画
    draw_bar(&filename, total, total, start, true);

    println!();
    Ok(())
}
