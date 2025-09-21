use crate::wbi::WbiSigner;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use reqwest::{Client, Proxy, Url};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex, RawCookie};
use std::sync::Arc;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use tokio::time::{sleep, Duration};

#[derive(Clone)]
pub struct BiliClient {
    http: Client,
    cookie_header: Option<String>,
    jar: Option<Arc<CookieStoreMutex>>,
}

impl BiliClient {
    pub fn new(user_agent: String, referer: String, cookies: Option<String>, proxy: Option<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&user_agent).unwrap_or(HeaderValue::from_static(
                "bilibili-dl/0.1",
            )),
        );
        headers.insert(REFERER, HeaderValue::from_str(&referer).unwrap());

        let mut builder = Client::builder()
            .default_headers(headers.clone())
            .cookie_store(true)
            .http1_only() // some networks/servers reset h2; prefer h1 for stability
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(user_agent);

        if let Some(p) = proxy {
            builder = builder.proxy(Proxy::all(&p)?);
        }

        // Simple Netscape cookie file: extract name=value pairs for api domain
        let mut cookie_header: Option<String> = None;
        let mut jar: Option<Arc<CookieStoreMutex>> = None;
        if let Some(path) = cookies {
            if let Ok(h) = build_cookie_header_from_netscape(&path) {
                use reqwest::header::COOKIE;
                let mut merged = headers;
                merged.insert(COOKIE, HeaderValue::from_str(&h).unwrap());
                builder = builder.default_headers(merged);
                cookie_header = Some(h);
            }
            // Also load cookies into a shared cookie jar
            let store = CookieStore::default();
            let jar_arc = Arc::new(CookieStoreMutex::new(store));
            if let Ok(cnt) = load_netscape_into_jar(&jar_arc, &path) {
                if cnt > 0 {
                    builder = builder.cookie_provider(jar_arc.clone());
                    jar = Some(jar_arc);
                }
            }
        }

        let http = builder.build()?;
        Ok(Self { http, cookie_header, jar })
    }

    pub async fn resolve_bvid_and_cid(&self, input: &str, page: u32) -> Result<(String, u64)> {
        let (bvid, page_from_url) = self
            .parse_bvid_and_page(input)
            .await
            .context("parse input")?;
        // if URL had ?p=, use it unless user passed -p (we can't detect explicit flag; use heuristic: if page==1 and URL has p>0, use it)
        let page = if page == 1 { page_from_url.unwrap_or(1) } else { page };
        // fetch view for cids
        let url = Url::parse_with_params(
            "https://api.bilibili.com/x/web-interface/view",
            &[("bvid", bvid.clone())],
        )?;
        let view: ViewResp = self.get_json_retry(url).await?;
        let data = view.data.ok_or_else(|| anyhow!("view data missing"))?;
        let idx = (page.saturating_sub(1)) as usize;
        let page_item = data
            .pages
            .get(idx)
            .ok_or_else(|| anyhow!("page {} not found", page))?;
        Ok((bvid, page_item.cid))
    }

    async fn parse_bvid_and_page(&self, input: &str) -> Result<(String, Option<u32>)> {
        if let Some(bv) = extract_bvid(input) {
            let p = extract_page_param(input);
            return Ok((bv, p));
        }
        // Try as URL: follow redirects (for b23.tv) and then extract
        if let Ok(mut url) = Url::parse(input) {
            // Quick page param before request
            let p = url
                .query_pairs()
                .find(|(k, _)| k == "p")
                .and_then(|(_, v)| v.parse::<u32>().ok());
            let resp = self.http.get(url.clone()).send().await?;
            url = resp.url().clone();
            if let Some(bv) = extract_bvid(url.as_str()) {
                return Ok((bv, p));
            }
        }
        Err(anyhow!("BV id not found in input"))
    }

    pub async fn get_title(&self, bvid: &str) -> Result<String> {
        let url = Url::parse_with_params(
            "https://api.bilibili.com/x/web-interface/view",
            &[("bvid", bvid.to_string())],
        )?;
        let view: ViewResp = self.get_json_retry(url).await?;
        let data = view.data.ok_or_else(|| anyhow!("view data missing"))?;
        Ok(data.title)
    }

    pub async fn get_playurl(
        &self,
        bvid: &str,
        cid: u64,
        quality: Option<u32>,
        fnval: u32,
    )
    -> Result<PlayUrlResp> {
        let signer = WbiSigner::fetch(&self.http).await?;
        let mut params = vec![
            ("bvid".to_string(), bvid.to_string()),
            ("cid".to_string(), cid.to_string()),
            ("fnval".to_string(), fnval.to_string()),
            ("fourk".to_string(), "1".into()),
            ("hires".to_string(), "1".into()),
        ];
        if let Some(qn) = quality {
            params.push(("qn".into(), qn.to_string()));
        }
        let (params, _wts, w_rid) = signer.sign(params);
        let mut url = Url::parse("https://api.bilibili.com/x/player/wbi/playurl")?;
        {
            let mut qp = url.query_pairs_mut();
            for (k, v) in &params {
                qp.append_pair(k, v);
            }
            qp.append_pair("w_rid", &w_rid);
        }
        let parsed: PlayUrlResp = self.get_json_retry(url).await?;
        if parsed.code != 0 {
            return Err(anyhow!(
                "playurl api error code {}: {}",
                parsed.code,
                parsed.message.clone().unwrap_or_default()
            ));
        }
        Ok(parsed)
    }

    async fn get_json_retry<T: serde::de::DeserializeOwned>(&self, url: Url) -> Result<T> {
        let mut last_err = None;
        for (i, delay_ms) in [0u64, 500, 1500].into_iter().enumerate() {
            if delay_ms > 0 { sleep(Duration::from_millis(delay_ms)).await; }
            let res = self.http.get(url.clone()).send().await;
            match res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp.json::<T>().await?);
                    }
                    last_err = Some(anyhow!("http status {}", status));
                }
                Err(e) => { last_err = Some(anyhow!("send error: {} (attempt {})", e, i+1)); }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("request failed")))
    }
}

impl BiliClient {
    pub fn cookie_header(&self) -> Option<&str> { self.cookie_header.as_deref() }
    pub fn cookie_jar(&self) -> Option<Arc<CookieStoreMutex>> { self.jar.clone() }
}

pub fn select_streams(
    dash: &Dash,
    prefer_codec: Option<&str>,
    max_height: Option<i32>,
    want_video: bool,
    want_audio: bool,
) -> (Option<DashVideo>, Option<DashAudio>) {
    let pref = prefer_codec.unwrap_or("avc1").to_ascii_lowercase();
    let mut vsel = None;
    if want_video {
        let mut videos = dash.video.clone();
        // Prefer higher height/id
        videos.sort_by(|a, b| a.height.cmp(&b.height).then(a.id.cmp(&b.id)));
        videos.reverse();
        if let Some(h) = max_height {
            videos.retain(|v| v.height.map(|x| x <= h).unwrap_or(true));
        }
        // Prefer codec
        vsel = videos
            .iter()
            .find(|v| v.codecs.to_ascii_lowercase().contains(&pref))
            .cloned()
            .or_else(|| videos.first().cloned());
    }

    let mut asel = None;
    if want_audio {
        let mut audios = dash.audio.clone().unwrap_or_default();
        audios.sort_by(|a, b| a.id.cmp(&b.id));
        audios.reverse();
        asel = audios.first().cloned();
    }
    (vsel, asel)
}

pub fn select_streams_with_format(dash: &Dash, fmt: &str) -> (Option<DashVideo>, Option<DashAudio>) {
    // Alternatives separated by '/'
    for alt in fmt.split('/') {
        // Split by '+' to see if need video+audio
        let parts: Vec<&str> = alt.split('+').collect();
        let (want_video, want_audio) = match parts.as_slice() {
            [v, a] => (is_video_token(v), is_audio_token(a)),
            [single] => {
                if is_best_token(single) { (true, true) }
                else if is_video_token(single) { (true, false) }
                else if is_audio_token(single) { (false, true) }
                else { (true, true) }
            }
            _ => (true, true),
        };
        let vfilter = parse_filter(parts.get(0).copied());
        let afilter = if parts.len() > 1 { parse_filter(parts.get(1).copied()) } else { Default::default() };

        let vsel = if want_video { pick_video(dash, &vfilter) } else { None };
        let asel = if want_audio { pick_audio(dash, &afilter) } else { None };
        let ok_v = !want_video || vsel.is_some();
        let ok_a = !want_audio || asel.is_some();
        if ok_v && ok_a {
            return (vsel, asel);
        }
    }
    (None, None)
}

#[derive(Default, Clone)]
struct Filter {
    max_height: Option<i32>,
    min_height: Option<i32>,
    vcodec_eq: Option<String>,
    vcodec_prefix: Option<String>,
    acodec_eq: Option<String>,
}

fn parse_filter(token: Option<&str>) -> Filter {
    let mut f = Filter::default();
    let Some(tok) = token else { return f; };
    // extract bracket constraints like bestvideo[height<=1080][vcodec^=av01]
    for cap in Regex::new(r"\[(.*?)\]").unwrap().captures_iter(tok) {
        let expr = cap.get(1).unwrap().as_str();
        if let Some(v) = expr.strip_prefix("height<=") {
            if let Ok(h) = v.parse::<i32>() { f.max_height = Some(h); }
        } else if let Some(v) = expr.strip_prefix("height>=") {
            if let Ok(h) = v.parse::<i32>() { f.min_height = Some(h); }
        } else if let Some(v) = expr.strip_prefix("vcodec^") {
            if let Some(v2) = v.strip_prefix("=") { f.vcodec_prefix = Some(v2.to_string()); }
        } else if let Some(v) = expr.strip_prefix("vcodec=") {
            f.vcodec_eq = Some(v.to_string());
        } else if let Some(v) = expr.strip_prefix("acodec=") {
            f.acodec_eq = Some(v.to_string());
        }
    }
    // quick codec hints directly in token (e.g., "+av01")
    for c in ["avc1", "hev1", "h265", "av01", "av1"] {
        if tok.to_ascii_lowercase().contains(c) {
            let cc = if c=="h265" { "hev1" } else { c };
            f.vcodec_prefix = Some(cc.to_string());
        }
    }
    f
}

fn pick_video(dash: &Dash, f: &Filter) -> Option<DashVideo> {
    let mut vids = dash.video.clone();
    vids.retain(|v| {
        if let Some(h) = f.max_height { if v.height.map(|x| x > h).unwrap_or(false) { return false; } }
        if let Some(h) = f.min_height { if v.height.map(|x| x < h).unwrap_or(false) { return false; } }
        if let Some(ref eq) = f.vcodec_eq { if v.codecs != *eq { return false; } }
        if let Some(ref pf) = f.vcodec_prefix { if !v.codecs.to_ascii_lowercase().starts_with(&pf.to_ascii_lowercase()) { return false; } }
        true
    });
    vids.sort_by(|a,b| a.height.cmp(&b.height).then(a.id.cmp(&b.id)));
    vids.pop()
}

fn pick_audio(dash: &Dash, f: &Filter) -> Option<DashAudio> {
    let mut auds = dash.audio.clone().unwrap_or_default();
    auds.retain(|a| {
        if let Some(ref eq) = f.acodec_eq { if a.codecs != *eq { return false; } }
        true
    });
    auds.sort_by(|a,b| a.id.cmp(&b.id));
    auds.pop()
}

fn is_best_token(s: &str) -> bool { let l = s.to_ascii_lowercase(); l=="best" || l=="b" }
fn is_video_token(s: &str) -> bool { let l = s.to_ascii_lowercase(); l.starts_with("bestvideo") || l.starts_with("bv") }
fn is_audio_token(s: &str) -> bool { let l = s.to_ascii_lowercase(); l.starts_with("bestaudio") || l.starts_with("ba") }

pub fn extract_bvid(input: &str) -> Option<String> {
    // direct BV id
    let re_bv = Regex::new(r"BV[0-9A-Za-z]{10}").ok()?;
    if let Some(m) = re_bv.find(input) {
        return Some(m.as_str().to_string());
    }
    None
}

pub fn extract_page_param(input: &str) -> Option<u32> {
    if let Ok(url) = Url::parse(input) {
        return url
            .query_pairs()
            .find(|(k, _)| k == "p")
            .and_then(|(_, v)| v.parse::<u32>().ok());
    }
    None
}

fn build_cookie_header_from_netscape(path: &str) -> Result<String> {
    let file = File::open(path).context("open cookies file")?;
    let reader = BufReader::new(file);
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 7 { continue; }
        let name = parts[5].trim().to_string();
        let value = parts[6].trim().to_string();
        // Keep key cookies commonly needed
        if matches!(name.as_str(), "SESSDATA" | "bili_jct" | "buvid3" | "DedeUserID" | "DedeUserID__ckMd5") {
            pairs.push((name, value));
        }
    }
    if pairs.is_empty() { return Err(anyhow!("no useful cookies found")); }
    Ok(pairs.into_iter().map(|(k,v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("; "))
}

fn load_netscape_into_jar(jar: &Arc<CookieStoreMutex>, path: &str) -> Result<usize> {
    let file = File::open(path).context("open cookies file")?;
    let reader = BufReader::new(file);
    let mut count = 0usize;
    for line in reader.lines() {
        let line = line?;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 7 { continue; }
        let domain = parts[0].trim();
        let path = parts[2].trim();
        let secure = parts[3].trim().eq_ignore_ascii_case("TRUE");
        let name = parts[5].trim();
        let value = parts[6].trim();
        if name.is_empty() { continue; }
        let origin = format!("https://{}", domain.trim_start_matches('.'));
        if let Ok(url) = Url::parse(&origin) {
            let p = if path.is_empty() { "/" } else { path };
            let mut rc = RawCookie::new(name.to_string(), value.to_string());
            rc.set_path(p.to_string());
            rc.set_domain(domain.to_string());
            if secure { rc.set_secure(true); }
            if let Ok(mut guard) = jar.lock() {
                let _ = guard.store_response_cookies(std::iter::once(rc), &url);
                count += 1;
            }
        }
    }
    Ok(count)
}

// ==== Types ====

#[derive(Debug, Deserialize)]
pub struct PlayUrlResp {
    pub code: i32,
    pub message: Option<String>,
    pub data: Option<PlayUrlData>,
}

#[derive(Debug, Deserialize)]
pub struct PlayUrlData {
    pub dash: Option<Dash>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Dash {
    pub video: Vec<DashVideo>,
    pub audio: Option<Vec<DashAudio>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DashVideo {
    pub id: i32,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub codecs: String,
    pub height: Option<i32>,
    pub bandwidth: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DashAudio {
    pub id: i32,
    #[serde(rename = "baseUrl")]
    pub base_url: String,
    pub codecs: String,
    pub bandwidth: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ViewResp {
    data: Option<ViewData>,
}

#[derive(Debug, Deserialize)]
struct ViewData {
    title: String,
    pages: Vec<ViewPage>,
}

#[derive(Debug, Deserialize)]
struct ViewPage {
    cid: u64,
}
