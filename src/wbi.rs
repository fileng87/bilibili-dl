use anyhow::{anyhow, Result};
use md5;
use regex::Regex;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone)]
pub struct WbiSigner {
    mixin_key: String,
}

impl WbiSigner {
    pub async fn fetch(client: &Client) -> Result<Self> {
        let nav: NavResp = get_json_retry(client, "https://api.bilibili.com/x/web-interface/nav").await?;

        let data = nav.data.ok_or_else(|| anyhow!("nav data missing"))?;
        let img_key = extract_key(&data.wbi_img.img_url)?;
        let sub_key = extract_key(&data.wbi_img.sub_url)?;
        let mixin_key = mixin_key(&format!("{}{}", img_key, sub_key));
        Ok(Self { mixin_key })
    }

    pub fn sign(&self, mut params: Vec<(String, String)>) -> (Vec<(String, String)>, u64, String) {
        let wts = now_ts();
        params.push(("wts".to_string(), wts.to_string()));

        // sanitize values
        for (_, v) in params.iter_mut() {
            *v = sanitize(v);
        }

        // sort by key
        params.sort_by(|a, b| a.0.cmp(&b.0));

        // build query
        let q = params
            .iter()
            .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        let digest = md5::compute([q.as_bytes(), self.mixin_key.as_bytes()].concat());
        let w_rid = format!("{:x}", digest);
        (params, wts, w_rid)
    }

    /// Test helper: construct a signer from a known mixin_key
    pub fn for_test(mixin_key: &str) -> Self { Self { mixin_key: mixin_key.to_string() } }
}

async fn get_json_retry<T: serde::de::DeserializeOwned>(client: &Client, url: &str) -> Result<T> {
    let mut last_err = None;
    for delay_ms in [0u64, 500, 1500] {
        if delay_ms > 0 { sleep(Duration::from_millis(delay_ms)).await; }
        match client.get(url).send().await {
            Ok(resp) => {
                if resp.status().is_success() { return Ok(resp.json::<T>().await?); }
                last_err = Some(anyhow!("http status {}", resp.status()));
            }
            Err(e) => { last_err = Some(anyhow!("send error: {}", e)); }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow!("request failed")))
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn url_encode(s: &str) -> String {
    // Use Url::parse_with_params to escape, but simpler percent-encoding via reqwest/Url is fine
    // Build a dummy URL and extract the query
    let mut u = Url::parse("http://x.local/").unwrap();
    u.query_pairs_mut().append_pair("k", s);
    let q = u.query().unwrap_or("");
    q.trim_start_matches("k=").to_string()
}

pub fn sanitize(v: &str) -> String {
    // Remove characters: ! ' ( ) * per docs (commonly used set; ~ also sometimes removed)
    let re = Regex::new(r"[!'()*~]").unwrap();
    re.replace_all(v, "").to_string()
}

pub fn extract_key(url: &str) -> Result<String> {
    // e.g., https://i0.hdslb.com/bfs/wbi/abcd1234efgh5678.png -> abcd1234efgh5678
    let re = Regex::new(r"/([a-zA-Z0-9]+)\.(png|jpg)$").unwrap();
    let caps = re
        .captures(url)
        .ok_or_else(|| anyhow!("failed to parse wbi key from url: {}", url))?;
    Ok(caps.get(1).unwrap().as_str().to_string())
}

pub fn mixin_key(seed: &str) -> String {
    // Table from public reverse engineering; take first 32 chars
    const TAB: [usize; 64] = [
        46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42,
        19, 29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51,
        30, 4, 22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 20, 34, 36, 44, 52,
    ];
    let chars: Vec<char> = seed.chars().collect();
    let mixed: String = TAB
        .iter()
        .filter_map(|&i| chars.get(i).copied())
        .collect();
    mixed.chars().take(32).collect()
}

#[derive(Debug, Deserialize)]
struct NavResp {
    data: Option<NavData>,
}

#[derive(Debug, Deserialize)]
struct NavData {
    wbi_img: WbiImg,
}

#[derive(Debug, Deserialize)]
struct WbiImg {
    img_url: String,
    sub_url: String,
}
