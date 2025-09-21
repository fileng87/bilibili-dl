# bilibili-dl

Minimal Rust CLI to download Bilibili videos via the WBI playurl API.

- Async HTTP (`reqwest` + rustls), progress bar (`indicatif`)
- WBI signing implemented; keys pulled from `x/web-interface/nav`
- DASH download: separate video/audio .m4s, optional ffmpeg mux (copy)
- yt-dlp–style flags (subset): `-F`, `-f`, `-o`, `--cookies`, `--proxy`, `--continue`, `--merge-output-format`, `--no-cleanup`

Quick Start
- Build: `cargo build --release`
- List formats: `bilibili-dl <URL|BV...> -F`
- Best video+audio: `bilibili-dl <URL|BV...> -f bestvideo+bestaudio -o "%(title)s.%(ext)s"`
- Only print URLs (no download): `bilibili-dl <URL|BV...> --print-only`

URL Handling
- Accepts BV id or full URLs, including share links with extra params.
- Follows b23.tv short links (HTTP redirect).
- If URL has `?p=N` and you did not pass `-p`, it uses that page.

Format Selection (-f)
- Alternatives separated by `/`, first matching wins.
- Combos with `+`:
  - `bestvideo+bestaudio` (alias: `bv*+ba`, `bv+ba`)
  - `bestvideo`/`bestaudio` or `bv`/`ba` for single track
  - `best` or `b` uses best A/V
- Filters inside `[]` (basic subset):
  - `height<=1080`, `height>=720`
  - `vcodec=` exact, `vcodec^=` prefix (e.g., `vcodec^=av01`)
  - audio: `acodec=` exact
- Codec hints also work when written inline: `avc1`, `hev1` (`h265`), `av01`, `av1`.

Other Useful Flags
- `-o, --output` template (yt-dlp style): supports `%(title)s`, `%(id)s`(BV), `%(cid)s`, `%(ext)s`
- `--merge-output-format` container: `mp4` (default) or `mkv`
- `--cookies <netscape.txt>`: reads key cookies (SESSDATA 等) to unlock higher qualities
- `--proxy <url>`: e.g. `http://127.0.0.1:7890`
- `--continue`: resume partial `.m4s` via HTTP Range
- `--no-cleanup`: by default, successful mux removes `.m4s`; this flag keeps them
- `--no-mux`: skip mux and keep separate `.m4s`

Examples
- List then pick: `bilibili-dl https://www.bilibili.com/video/BVxxxx -F`
- Prefer AV1 up to 1080p: `bilibili-dl BVxxxx -f "bestvideo[height<=1080][vcodec^=av01]+bestaudio/best" -o "%(title)s.%(ext)s"`
- Audio only: `bilibili-dl BVxxxx -f ba -o "%(title)s.%(ext)s" --merge-output-format mkv`
- Share link: `bilibili-dl "https://www.bilibili.com/video/BV.../?share_source=copy_web&vd_source=..." -f best`

Notes
- High qualities may require login/VIP. This tool does not handle login UI; provide cookies if needed.
- API can change. If something breaks, check SocialSisterYi’s bilibili-API-collect.
- `.m4s` are fragmented MP4 tracks from DASH; ffmpeg merges them with `-c copy`.

Acknowledgements
- SocialSisterYi/bilibili-API-collect project: https://github.com/SocialSisterYi/bilibili-API-collect
