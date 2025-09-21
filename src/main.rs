use anyhow::{Context, Result};
use clap::Parser;

use bilibili_dl::{cli, bilibili, downloader};
use bilibili_dl::util::{parse_format, expand_template, sanitize_filename};

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    if args.list_formats {
        run_list_formats(args).await
    } else if args.print_only {
        run_and_print(args).await
    } else {
        run_and_download(args).await
    }
}

async fn run_and_print(args: cli::Args) -> Result<()> {
    let client = bilibili::BiliClient::new(
        args.user_agent.clone(),
        args.referer.clone(),
        args.cookies.clone(),
        args.proxy.clone(),
    )?;
    let (bvid, cid) = client
        .resolve_bvid_and_cid(&args.input, args.page)
        .await
        .context("resolve BV and CID failed")?;

    let play = client
        .get_playurl(&bvid, cid, args.quality, args.fnval)
        .await
        .context("get playurl failed")?;

    if let Some(dash) = play.data.and_then(|d| d.dash) {
        let (vsel, asel) = if let Some(ref fstr) = args.format {
            bilibili::select_streams_with_format(&dash, fstr)
        } else {
            let fmt = parse_format(&args.format, args.prefer_codec.as_deref());
            bilibili::select_streams(&dash, fmt.prefer_codec.as_deref(), fmt.max_height, fmt.want_video, fmt.want_audio)
        };
        println!("bvid: {}  cid: {}", bvid, cid);
        if let Some(v) = vsel {
            println!("video[{} {} {}p]: {}", v.id, v.codecs, v.height.unwrap_or(0), v.base_url);
        }
        if let Some(a) = asel {
            println!("audio[{} {}]: {}", a.id, a.codecs, a.base_url);
        }
    } else {
        println!("No DASH data available (maybe login required or invalid params)");
    }
    Ok(())
}

async fn run_list_formats(args: cli::Args) -> Result<()> {
    let client = bilibili::BiliClient::new(
        args.user_agent.clone(),
        args.referer.clone(),
        args.cookies.clone(),
        args.proxy.clone(),
    )?;
    let (bvid, cid) = client
        .resolve_bvid_and_cid(&args.input, args.page)
        .await
        .context("resolve BV and CID failed")?;

    let play = client
        .get_playurl(&bvid, cid, args.quality, args.fnval)
        .await
        .context("get playurl failed")?;

    let Some(dash) = play.data.and_then(|d| d.dash) else {
        eprintln!("No DASH data returned. Try with cookies or other quality.");
        return Ok(());
    };

    println!("Formats for {} (cid {}):", bvid, cid);
    println!("ID   type   res    codec         br (kbps)");
    println!("---- ------ ------ ------------- ----------");
    let mut vids = dash.video.clone();
    vids.sort_by(|a,b| a.height.cmp(&b.height).then(a.id.cmp(&b.id)));
    vids.reverse();
    for v in vids.iter() {
        let h = v.height.unwrap_or(0);
        let br = v.bandwidth.map(|x| x/1000).unwrap_or(0);
        println!("{:<4} video  {:>4}p {:<13} {:>10}", v.id, h, v.codecs, br);
    }
    if let Some(auds) = dash.audio.clone() {
        let mut auds = auds;
        auds.sort_by(|a,b| a.id.cmp(&b.id));
        auds.reverse();
        for a in auds.iter() {
            let br = a.bandwidth.map(|x| x/1000).unwrap_or(0);
            println!("{:<4} audio   ----  {:<13} {:>10}", a.id, a.codecs, br);
        }
    }
    Ok(())
}

async fn run_and_download(args: cli::Args) -> Result<()> {
    let client = bilibili::BiliClient::new(
        args.user_agent.clone(),
        args.referer.clone(),
        args.cookies.clone(),
        args.proxy.clone(),
    )?;
    let (bvid, cid) = client
        .resolve_bvid_and_cid(&args.input, args.page)
        .await
        .context("resolve BV and CID failed")?;

    let play = client
        .get_playurl(&bvid, cid, args.quality, args.fnval)
        .await
        .context("get playurl failed")?;

    let title = client
        .get_title(&bvid)
        .await
        .unwrap_or_else(|_| bvid.clone());

    let Some(dash) = play.data.and_then(|d| d.dash) else {
        eprintln!("No DASH data returned. Try a different quality, or with cookies.");
        return Ok(());
    };

    let (vsel, asel) = if let Some(ref fstr) = args.format {
        bilibili::select_streams_with_format(&dash, fstr)
    } else {
        let fmt = parse_format(&args.format, args.prefer_codec.as_deref());
        bilibili::select_streams(&dash, fmt.prefer_codec.as_deref(), fmt.max_height, fmt.want_video, fmt.want_audio)
    };
    if vsel.is_none() && asel.is_none() {
        eprintln!("No suitable streams found.");
        return Ok(());
    }

    let container = args
        .merge_output_format
        .clone()
        .unwrap_or_else(|| "mp4".to_string());
    let out_stem = if let Some(tpl) = args.output.clone().or(args.out.clone()) {
        expand_template(&tpl, &title, &bvid, cid, &container)
    } else {
        sanitize_filename(&title)
    };

    let mut video_path = None;
    let mut audio_path = None;

    if let Some(v) = vsel {
        let vp = format!("{}-v-{}.m4s", out_stem, v.id);
        downloader::download_with_progress(&v.base_url, &vp, &args.user_agent, &args.referer, args.resume).await?;
        video_path = Some(vp);
    }

    if let Some(a) = asel {
        let ap = format!("{}-a-{}.m4s", out_stem, a.id);
        downloader::download_with_progress(&a.base_url, &ap, &args.user_agent, &args.referer, args.resume).await?;
        audio_path = Some(ap);
    }

    if args.no_mux {
        println!("Saved tracks. Skipping mux (--no-mux). Done.");
        return Ok(());
    }

    if let (Some(vp), Some(ap)) = (video_path.as_deref(), audio_path.as_deref()) {
        let out_path = format!("{}.{}", out_stem, container);
        match downloader::ffmpeg_mux(vp, ap, &out_path).await {
            Ok(_) => {
                println!("Muxed -> {}", out_path);
                let do_cleanup = if args.no_cleanup { false } else { args.cleanup };
                if do_cleanup {
                    let _ = tokio::fs::remove_file(vp).await;
                    let _ = tokio::fs::remove_file(ap).await;
                }
            }
            Err(e) => {
                eprintln!("ffmpeg mux failed: {e}. Tracks left as-is");
            }
        }
    }

    Ok(())
}

// helpers moved to library (util.rs)
