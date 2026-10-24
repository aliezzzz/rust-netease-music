//! Opt-in live network probe. Uses isolated data when NCM_DATA_DIR is supplied.
use music_core::{api::Query, Command, Core, Paths, Playback};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let core = Core::start(Paths::discover()?)?;
    let page = core
        .api
        .query(Query {
            kind: "search_song".into(),
            text: "晴天".into(),
            id: 0,
            offset: 0,
            limit: 6,
        })
        .await?;
    let mut success = false;
    for track in page.tracks {
        println!("Trying {} — {} ({})", track.name, track.artist, track.id);
        if let Err(error) = core
            .command(Command::Replace {
                tracks: vec![track],
                index: 0,
            })
            .await
        {
            println!("Skipped: {error}");
            continue;
        }
        let mut updates = core.subscribe();
        let played = tokio::time::timeout(Duration::from_secs(40), async {
            loop {
                let s = updates.borrow_and_update().clone();
                if s.playback == Playback::Error {
                    println!("Playback error: {:?}", s.error);
                    return false;
                }
                if s.playback == Playback::Playing && s.position_ms >= 2000 {
                    println!(
                        "Real network playback: {} / {} ms",
                        s.position_ms, s.duration_ms
                    );
                    return true;
                }
                if updates.changed().await.is_err() {
                    return false;
                }
            }
        })
        .await
        .unwrap_or(false);
        if played {
            success = true;
            break;
        }
    }
    core.command(Command::Shutdown).await?;
    anyhow::ensure!(success, "None of the live network tracks reached playback");
    Ok(())
}
