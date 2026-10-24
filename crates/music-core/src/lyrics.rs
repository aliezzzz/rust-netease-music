use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
    pub translation: String,
}

fn parse(lines: &[String]) -> BTreeMap<u64, String> {
    static TIME: OnceLock<Regex> = OnceLock::new();
    static OFFSET: OnceLock<Regex> = OnceLock::new();
    let time = TIME.get_or_init(|| Regex::new(r"\[(\d+):(\d{2})(?:[.:](\d{1,3}))?\]").unwrap());
    let offset = OFFSET.get_or_init(|| Regex::new(r"\[offset:([+-]?\d+)\]").unwrap());
    let offset_ms = lines
        .iter()
        .find_map(|line| offset.captures(line))
        .and_then(|c| c[1].parse::<i64>().ok())
        .unwrap_or(0);
    let mut result = BTreeMap::new();
    for line in lines {
        let text = time.replace_all(line, "").trim().to_owned();
        for c in time.captures_iter(line) {
            let fraction = c
                .get(3)
                .map(|m| {
                    m.as_str().parse::<u64>().unwrap_or(0) * 10u64.pow(3 - m.as_str().len() as u32)
                })
                .unwrap_or(0);
            let ms = (c[1].parse::<u64>().unwrap_or(0) * 60 + c[2].parse::<u64>().unwrap_or(0))
                * 1000
                + fraction;
            result.insert((ms as i64 + offset_ms).max(0) as u64, text.clone());
        }
    }
    result
}

pub fn merge(original: &[String], translated: &[String]) -> Vec<LyricLine> {
    let translations = parse(translated);
    parse(original)
        .into_iter()
        .map(|(time_ms, text)| LyricLine {
            time_ms,
            text,
            translation: translations.get(&time_ms).cloned().unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_offset_and_translations() {
        let lines = merge(
            &[
                "[offset:-100]".into(),
                "[00:01.2][01:02.345]一句歌词".into(),
            ],
            &["[00:01.10]translation".into()],
        );
        assert_eq!(lines[0].time_ms, 1100);
        assert_eq!(lines[0].translation, "translation");
        assert_eq!(lines[1].time_ms, 62245);
    }
}
