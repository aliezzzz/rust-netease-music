use music_core::{api::Query, Api, Paths};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let paths = Paths::discover()?;
    let api = Api::new(paths, "")?;
    if std::env::var_os("NCM_PROBE_DETAILS").is_some() {
        let raw: serde_json::Value =
            serde_json::from_str(&api.client().search("晴天".into(), 1, 0, 1).await?)?;
        println!(
            "Search album metadata: {}",
            raw["result"]["songs"][0]["album"]
        );
    }
    let mut failures = Vec::new();
    let mut details = Vec::new();
    for kind in [
        "discover",
        "albums",
        "charts",
        "search_song",
        "search_artist",
        "search_album",
        "search_playlist",
        "search_lyrics",
    ] {
        let result = api
            .query(Query {
                kind: kind.into(),
                id: 0,
                text: "周杰伦".into(),
                offset: 0,
                limit: 6,
            })
            .await;
        match result {
            Ok(page) => {
                println!(
                    "{kind}: {} cards / {} tracks",
                    page.cards.len(),
                    page.tracks.len()
                );
                if page.cards.is_empty() && page.tracks.is_empty() {
                    failures.push(kind);
                }
                if ["discover", "albums", "search_artist"].contains(&kind) {
                    if let Some(card) = page.cards.first() {
                        details.push(card.clone());
                    }
                }
                if kind == "search_song" {
                    println!(
                        "Search covers: {} / {}",
                        page.tracks.iter().filter(|t| !t.cover.is_empty()).count(),
                        page.tracks.len()
                    );
                    if let Some(track) = page.tracks.first() {
                        println!("First search cover: {}", track.cover);
                        match api.lyrics(track.id).await {
                            Ok(lines) => println!("Lyrics: {} lines", lines.len()),
                            Err(error) => {
                                println!("Lyrics: ERROR {error:#}");
                                failures.push("lyrics");
                            }
                        }
                    }
                }
            }
            Err(error) => {
                println!("{kind}: ERROR {error:#}");
                failures.push(kind);
            }
        }
    }
    for card in details {
        let page = api
            .query(Query {
                kind: card.kind.clone(),
                id: card.id,
                text: card.name.clone(),
                offset: 0,
                limit: 6,
            })
            .await;
        match page {
            Ok(page) if !page.tracks.is_empty() => println!(
                "Detail {} {}: {} tracks",
                card.kind,
                card.id,
                page.tracks.len()
            ),
            result => {
                println!("Detail {} {} failed: {result:?}", card.kind, card.id);
                failures.push("detail");
            }
        }
    }
    anyhow::ensure!(failures.is_empty(), "Live API failures: {:?}", failures);
    Ok(())
}
