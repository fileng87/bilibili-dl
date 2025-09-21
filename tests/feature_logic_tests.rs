use bilibili_dl::util::{parse_format, expand_template, sanitize_filename};
use bilibili_dl::bilibili::{select_streams_with_format, Dash, DashVideo, DashAudio};

fn sample_dash() -> Dash {
    Dash {
        video: vec![
            DashVideo { id: 120, base_url: "v2160_avc1".into(), codecs: "avc1.640032".into(), height: Some(2160), bandwidth: Some(12_000_000) },
            DashVideo { id: 80, base_url: "v1080_av01".into(), codecs: "av01.0.08M.10".into(), height: Some(1080), bandwidth: Some(5_000_000) },
            DashVideo { id: 64, base_url: "v720_hev1".into(), codecs: "hev1.1.6.L123".into(), height: Some(720), bandwidth: Some(3_000_000) },
        ],
        audio: Some(vec![
            DashAudio { id: 30216, base_url: "a128".into(), codecs: "mp4a.40.2".into(), bandwidth: Some(128_000) },
            DashAudio { id: 30232, base_url: "a320".into(), codecs: "mp4a.40.2".into(), bandwidth: Some(320_000) },
        ]),
    }
}

#[test]
fn format_parser_defaults() {
    let f = parse_format(&None, Some("avc1"));
    assert!(f.want_video && f.want_audio);
    assert_eq!(f.prefer_codec.as_deref(), Some("avc1"));
    assert_eq!(f.max_height, None);
}

#[test]
fn format_parser_height_and_codec() {
    // The simple parser supports inline hints and [height<=] anywhere in the string
    let inp = Some("best av01 [height<=1080]".to_string());
    let f = parse_format(&inp, None);
    assert!(f.want_video && f.want_audio);
    assert_eq!(f.max_height, Some(1080));
    assert_eq!(f.prefer_codec.as_deref(), Some("av01"));
}

#[test]
fn select_streams_av01_with_limit() {
    let dash = sample_dash();
    let (v,a) = select_streams_with_format(&dash, "bestvideo[height<=1080][vcodec^=av01]+bestaudio");
    assert!(v.is_some() && a.is_some());
    let v = v.unwrap();
    assert!(v.base_url.contains("av01"));
    assert!(v.height.unwrap() <= 1080);
}

#[test]
fn template_and_sanitize() {
    let stem = expand_template("%(title)s-%(id)s", "A<B>:\\bad/|name?*", "BVabc", 123, "mp4");
    // backslash is sanitized into underscore
    assert_eq!(stem, "A_B___bad__name__-BVabc");
    assert_eq!(sanitize_filename("  .x.  "), "x");
}
