use bilibili_dl::bilibili::{extract_bvid, extract_page_param, select_streams_with_format, Dash, DashVideo, DashAudio};

fn sample_dash() -> Dash {
    Dash {
        video: vec![
            DashVideo { id: 80, base_url: "v1080_avc1".into(), codecs: "avc1.640028".into(), height: Some(1080), bandwidth: Some(5_000_000) },
            DashVideo { id: 120, base_url: "v1080_hev1".into(), codecs: "hev1.1.6.L150".into(), height: Some(1080), bandwidth: Some(3_500_000) },
            DashVideo { id: 64, base_url: "v720_av01".into(), codecs: "av01.0.05M.08".into(), height: Some(720), bandwidth: Some(2_000_000) },
        ],
        audio: Some(vec![
            DashAudio { id: 30216, base_url: "a128".into(), codecs: "mp4a.40.2".into(), bandwidth: Some(128_000) },
            DashAudio { id: 30232, base_url: "a320".into(), codecs: "mp4a.40.2".into(), bandwidth: Some(320_000) },
        ]),
    }
}

#[test]
fn test_extract_bvid_from_share_url() {
    let url = "https://www.bilibili.com/video/BV1znWFzGEhi/?share_source=copy_web&vd_source=xxx";
    let bv = extract_bvid(url).expect("should parse BV");
    assert_eq!(bv, "BV1znWFzGEhi");
}

#[test]
fn test_extract_page_param() {
    let url = "https://www.bilibili.com/video/BV1xx?p=3";
    assert_eq!(extract_page_param(url), Some(3));
}

#[test]
fn test_select_streams_with_format_prefers_av1_and_filters_height() {
    let dash = sample_dash();
    let (v, a) = select_streams_with_format(&dash, "bestvideo[height<=1080][vcodec^=av01]+bestaudio/best");
    assert!(v.is_some());
    assert!(a.is_some());
    let v = v.unwrap();
    // our sample av01 track is 720p
    assert!(v.base_url.contains("av01"));
}

