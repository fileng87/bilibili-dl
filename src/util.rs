#[derive(Debug, Clone)]
pub struct FormatSel {
    pub want_video: bool,
    pub want_audio: bool,
    pub prefer_codec: Option<String>,
    pub max_height: Option<i32>,
}

pub fn parse_format(fmt: &Option<String>, prefer_codec_flag: Option<&str>) -> FormatSel {
    let mut sel = FormatSel { want_video: true, want_audio: true, prefer_codec: prefer_codec_flag.map(|s| s.to_string()), max_height: None };
    if let Some(s) = fmt {
        let lower = s.to_ascii_lowercase();
        if lower.contains("bestvideo+bestaudio") || lower.contains("bv*+ba") || lower == "bv+ba" { sel.want_video = true; sel.want_audio = true; }
        else if lower.contains("bestvideo") || lower.starts_with("bv") { sel.want_video = true; sel.want_audio = false; }
        else if lower.contains("bestaudio") || lower.starts_with("ba") { sel.want_video = false; sel.want_audio = true; }
        else if lower == "best" || lower == "b" { sel.want_video = true; sel.want_audio = true; }

        for c in ["avc1", "hev1", "h265", "av01", "av1"] {
            if lower.contains(c) { sel.prefer_codec = Some(if c=="h265" {"hev1".into()} else { c.into() }); }
        }
        if let Some(pos) = lower.find("height<=") {
            let num = lower[pos+8..].trim_start_matches(|ch: char| ch=='[' || ch=='=' || ch=='<').chars().take_while(|ch| ch.is_ascii_digit()).collect::<String>();
            if let Ok(h) = num.parse::<i32>() { sel.max_height = Some(h); }
        }
    }
    sel
}

pub fn expand_template(tpl: &str, title: &str, bvid: &str, cid: u64, ext: &str) -> String {
    let mut out = tpl.to_string();
    out = out.replace("%(title)s", &sanitize_filename(title));
    out = out.replace("%(id)s", bvid);
    out = out.replace("%(cid)s", &cid.to_string());
    out = out.replace("%(ext)s", ext);
    if !tpl.contains("%(ext)s") {
        sanitize_filename(&out)
    } else {
        out.trim_end_matches(&format!(".{}", ext)).to_string()
    }
}

pub fn sanitize_filename(s: &str) -> String {
    let bad = ["<", ">", ":", "\"", "\\", "/", "|", "?", "*"];
    let mut out = s.to_string();
    for b in &bad { out = out.replace(b, "_"); }
    out.trim().trim_matches('.').to_string()
}

