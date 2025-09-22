use clap::{ArgAction, Parser};

/// Simple Bilibili video downloader.
#[derive(Parser, Debug, Clone)]
#[command(author, version, about)]
pub struct Args {
    /// BV id or a full Bilibili URL
    pub input: String,

    /// Page number (1-based) for multi-part videos
    #[arg(short, long, default_value_t = 1)]
    pub page: u32,

    /// Desired quality id (e.g., 80=1080p, 64=720p). If absent, pick best.
    #[arg(short = 'q', long)]
    pub quality: Option<u32>,

    /// fnval flags. 4048 requests DASH. Adjust if you know what you’re doing.
    #[arg(long, default_value_t = 4048)]
    pub fnval: u32,

    /// Prefer codec (avc1|hev1|av01). Defaults to avc1 for compatibility.
    #[arg(long)]
    pub prefer_codec: Option<String>,

    /// Output (-o) template (yt-dlp style: %(title)s.%(ext)s). Overrides --out
    #[arg(short = 'o', long = "output")]
    pub output: Option<String>,

    /// Output file stem (without extension). Defaults to video title. (legacy)
    #[arg(long, hide = true)]
    pub out: Option<String>,

    /// Do not mux audio+video with ffmpeg; keep separate .m4s files
    #[arg(long, action = ArgAction::SetTrue)]
    pub no_mux: bool,

    /// Only print selected stream URLs, do not download
    #[arg(long, action = ArgAction::SetTrue)]
    pub print_only: bool,

    /// List available formats (like yt-dlp -F) and exit
    #[arg(short = 'F', long = "list-formats", action = ArgAction::SetTrue)]
    pub list_formats: bool,

    /// HTTP User-Agent header
    #[arg(long, default_value = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")]
    pub user_agent: String,

    /// HTTP Referer header
    #[arg(long, default_value = "https://www.bilibili.com")] 
    pub referer: String,

    // --- yt-dlp style compatibility flags (subset) ---
    /// Format selection like yt-dlp (e.g. "bestvideo+bestaudio/best", "best[height<=1080]", "bv*+ba")
    #[arg(short = 'f', long = "format")]
    pub format: Option<String>,

    /// Merge output format/container (mp4|mkv). Default mp4
    #[arg(long = "merge-output-format")] 
    pub merge_output_format: Option<String>,

    /// Cookies file in Netscape format
    #[arg(long = "cookies")]
    pub cookies: Option<String>,

    /// HTTP/SOCKS proxy URL, e.g. http://127.0.0.1:7890
    #[arg(long = "proxy")]
    pub proxy: Option<String>,

    /// Resume partially downloaded files
    #[arg(long = "continue", action = ArgAction::SetTrue)]
    pub resume: bool,

    /// Delete .m4s parts after successful mux (default: on). Use --no-cleanup to keep.
    #[arg(long, default_value_t = true)]
    pub cleanup: bool,

    /// Keep .m4s parts (disables --cleanup)
    #[arg(long = "no-cleanup", action = ArgAction::SetTrue)]
    pub no_cleanup: bool,

    /// Save cookies (Netscape format) after run
    #[arg(long = "save-cookies")]
    pub save_cookies: Option<String>,
}
