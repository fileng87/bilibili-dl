use anyhow::{anyhow, Context, Result};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use base64::Engine;
use dirs_next::data_local_dir;
use reqwest::Url;
use reqwest_cookie_store::{CookieStore, CookieStoreMutex, RawCookie};
use rusqlite::{Connection, OpenFlags};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(target_os = "windows")]
use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

#[cfg(target_os = "windows")]
pub fn load_from_browser(spec: &str) -> Result<(Arc<CookieStoreMutex>, Option<String>)> {
    let (browser, profile) = parse_spec(spec);
    let (db_path, local_state) = match browser.as_str() {
        "chrome" => (
            profile_path("Google/Chrome/User Data", profile, "Network/Cookies")?,
            profile_root("Google/Chrome/User Data")?.join("Local State"),
        ),
        "edge" => (
            profile_path("Microsoft/Edge/User Data", profile, "Network/Cookies")?,
            profile_root("Microsoft/Edge/User Data")?.join("Local State"),
        ),
        _ => return Err(anyhow!("unsupported browser: {}", browser)),
    };

    let key = decrypt_local_state_key(&local_state).context("decrypt Local State key")?;
    let tmp = copy_db_temp(&db_path)?;
    let conn = Connection::open_with_flags(&tmp, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt = conn.prepare("SELECT host_key, path, is_secure, name, encrypted_value FROM cookies")?;
    let mut rows = stmt.query([])?;

    let store = CookieStore::default();
    let jar = Arc::new(CookieStoreMutex::new(store));
    let mut header_pairs: Vec<(String, String)> = Vec::new();

    while let Some(row) = rows.next()? {
        let host: String = row.get(0)?;
        let path: String = row.get(1)?;
        let is_secure: i64 = row.get(2)?;
        let name: String = row.get(3)?;
        let enc: Vec<u8> = row.get(4)?;
        let val = decrypt_cookie_value(&enc, &key).or_else(|_| dpapi_unprotect(&enc))?;
        let value = String::from_utf8_lossy(&val).to_string();

        // write to jar
        let origin = format!("https://{}", host.trim_start_matches('.'));
        if let Ok(url) = Url::parse(&origin) {
            let mut rc = RawCookie::new(name.clone(), value.clone());
            rc.set_path(if path.is_empty() { "/".to_string() } else { path.clone() });
            rc.set_domain(host.clone());
            if is_secure != 0 { rc.set_secure(true); }
            if let Ok(mut guard) = jar.lock() {
                let _ = guard.store_response_cookies(std::iter::once(rc), &url);
            }
        }
        if matches!(name.as_str(), "SESSDATA" | "bili_jct" | "buvid3" | "DedeUserID" | "DedeUserID__ckMd5") {
            header_pairs.push((name, value));
        }
    }

    let header = if header_pairs.is_empty() { None } else { Some(header_pairs.into_iter().map(|(k,v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("; ")) };
    Ok((jar, header))
}

#[cfg(not(target_os = "windows"))]
pub fn load_from_browser(_spec: &str) -> Result<(Arc<CookieStoreMutex>, Option<String>)> {
    Err(anyhow!("--cookies-from-browser currently supported only on Windows"))
}

fn parse_spec(spec: &str) -> (String, String) {
    let mut parts = spec.splitn(2, ':');
    let browser = parts.next().unwrap_or("").to_ascii_lowercase();
    let profile = parts.next().unwrap_or("Default").to_string();
    (browser, profile)
}

fn profile_root(product: &str) -> Result<PathBuf> {
    let base = data_local_dir().ok_or_else(|| anyhow!("no local app data directory"))?;
    Ok(base.join(product))
}

fn profile_path(product: &str, profile: String, tail: &str) -> Result<PathBuf> {
    let root = profile_root(product)?;
    Ok(root.join(profile).join(tail))
}

fn copy_db_temp(src: &Path) -> Result<PathBuf> {
    let mut tmp = std::env::temp_dir();
    tmp.push(format!("bili_cookies_{}.sqlite", std::process::id()));
    fs::create_dir_all(tmp.parent().unwrap())?;
    fs::copy(src, &tmp).context("copy cookies db")?;
    Ok(tmp)
}

#[cfg(target_os = "windows")]
fn decrypt_local_state_key(path: &Path) -> Result<Vec<u8>> {
    let data = fs::read(path).context("read Local State")?;
    let json: serde_json::Value = serde_json::from_slice(&data)?;
    let enc_b64 = json["os_crypt"]["encrypted_key"].as_str().ok_or_else(|| anyhow!("no encrypted_key"))?;
    let mut enc = base64::engine::general_purpose::STANDARD.decode(enc_b64)?;
    // Strip DPAPI prefix "DPAPI"
    if enc.starts_with(b"DPAPI") { enc.drain(..5); }
    dpapi_unprotect(&enc)
}

#[cfg(target_os = "windows")]
fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>> {
    unsafe {
        let mut in_blob = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut _ };
        let mut out_blob = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptUnprotectData(&mut in_blob, None, None, None, None, 0, &mut out_blob);
        if ok.is_err() { return Err(anyhow!("CryptUnprotectData failed")); }
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        // LocalFree is not strictly necessary via this API; memory freed by system
        Ok(out)
    }
}

fn decrypt_cookie_value(enc: &[u8], key: &[u8]) -> Result<Vec<u8>> {
    if enc.len() > 3 && (&enc[0..3] == b"v10" || &enc[0..3] == b"v11") {
        let nonce = &enc[3..15];
        let ct = &enc[15..];
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let nonce = Nonce::from_slice(nonce);
        let plain = cipher.decrypt(nonce, ct).map_err(|_| anyhow!("AES-GCM decrypt failed"))?;
        Ok(plain)
    } else {
        Err(anyhow!("not aes-gcm"))
    }
}
