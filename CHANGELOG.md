# Changelog

## v0.2.1

- CI: gate Windows-only cookies code and dependency behind cfg(windows) so Linux runners compile cleanly
- No functional changes to the CLI; same features as v0.2.0

## v0.2.0

- Cookies: add `--cookies-from-browser` (Windows Chrome/Edge) to import cookies from the selected profile (SQLite + DPAPI/AES‑GCM)
- Cookies: add `--save-cookies` to export current cookie jar in Netscape format
- Share cookie jar across API requests and media downloads; also set Cookie header for API as fallback
- Networking: keep HTTP/1.1, timeouts, and small retries for stability

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
