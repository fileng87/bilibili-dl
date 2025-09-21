use bilibili_dl::wbi::{mixin_key, sanitize, url_encode, WbiSigner};

#[test]
fn test_mixin_key_rearranges_and_truncates() {
    // 64-char seed
    let seed = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+?";
    assert_eq!(seed.len(), 64);
    let mk = mixin_key(seed);
    assert_eq!(mk.len(), 32);
}

#[test]
fn test_sanitize_removes_specials() {
    let s = "a!b'c(d)e*f~g";
    let out = sanitize(s);
    assert_eq!(out, "abcdefg");
}

#[test]
fn test_url_encode_preserves_utf8() {
    let s = "中文 空格";
    let enc = url_encode(s);
    assert!(enc.contains('%'));
}

#[test]
fn test_sign_produces_rid_and_wts() {
    let signer = WbiSigner::for_test("0123456789abcdef0123456789abcdef");
    let (params, wts, w_rid) = signer.sign(vec![
        ("z".into(), "3".into()),
        ("a".into(), "1".into()),
        ("b".into(), "2".into()),
    ]);
    // Sorted by key and contains wts
    let keys: Vec<_> = params.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["a", "b", "wts", "z"]);
    assert!(wts > 0);
    assert_eq!(w_rid.len(), 32);
}

