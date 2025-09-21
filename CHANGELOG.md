# Changelog

## v0.1.0 (initial release)

- Rust CLI scaffold (Tokio, Clap)
- WBI signing via nav keys + MD5 mixin
- API client: view (cid/title), WBI playurl with retries/timeouts
- Downloader: progress bar, resume (Range), optional ffmpeg mux
- URL parsing: BV from links, b23 redirect, `?p=` auto select
- yt-dlp–style flags: `-F`, `-f`, `-o`, `--cookies`, `--proxy`, `--continue`, `--merge-output-format`, `--no-cleanup`, `--no-mux`
- Format parser: `bestvideo+bestaudio` and simple filters (`height<=`, `vcodec=`, `vcodec^=`)
- Tests: integration + CLI; optional online test (opt‑in)
- CI: GitHub Actions for build/test on Ubuntu and Windows

