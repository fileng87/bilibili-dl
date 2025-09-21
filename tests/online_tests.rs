use std::env;

use bilibili_dl::bilibili::BiliClient;

// Opt-in online test. Run with:
//   BILI_TEST_ONLINE=1 cargo test --test online_tests -- --nocapture
// Optional env:
//   BILI_TEST_BVID=BVxxxxxxxxxxx
//   BILI_TEST_PAGE=1
//   BILI_TEST_COOKIES=path/to/cookies.txt
//   BILI_TEST_PROXY=http://127.0.0.1:7890
//   BILI_TEST_FNVAL=4048

#[tokio::test]
#[ignore]
async fn online_view_and_playurl() {
    if env::var("BILI_TEST_ONLINE").ok().as_deref() != Some("1") {
        // Not enabled; skip
        return;
    }

    let bvid = env::var("BILI_TEST_BVID").unwrap_or_else(|_| "BV1znWFzGEhi".to_string());
    let page: u32 = env::var("BILI_TEST_PAGE").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let cookies = env::var("BILI_TEST_COOKIES").ok();
    let proxy = env::var("BILI_TEST_PROXY").ok();
    let fnval: u32 = env::var("BILI_TEST_FNVAL").ok().and_then(|s| s.parse().ok()).unwrap_or(4048);

    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36".to_string();
    let referer = "https://www.bilibili.com".to_string();

    let client = BiliClient::new(ua, referer, cookies, proxy).expect("client");

    let (bv, cid) = client.resolve_bvid_and_cid(&bvid, page).await.expect("view");
    assert!(bv.starts_with("BV"));

    let play = client.get_playurl(&bv, cid, None, fnval).await.expect("playurl");
    assert_eq!(play.code, 0, "playurl code should be 0");
    let dash = play.data.and_then(|d| d.dash);
    assert!(dash.is_some(), "expect DASH in response");
}

