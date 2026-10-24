// Adapted from src/audio/playlist.rs, Copyright (C) 2022 gmg137.
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{Repeat, Track};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Queue {
    pub tracks: Vec<Track>,
    pub current: Option<u64>,
    pub repeat: Repeat,
    pub shuffled: bool,
    order: Vec<u64>,
}

impl Queue {
    /// A display snapshot in the same order used by Next and Previous.
    pub fn ordered_tracks(&self) -> Vec<Track> {
        let tracks: HashMap<_, _> = self.tracks.iter().map(|track| (track.id, track)).collect();
        self.order
            .iter()
            .filter_map(|id| tracks.get(id).map(|track| (*track).clone()))
            .collect()
    }

    pub fn current(&self) -> Option<&Track> {
        self.current
            .and_then(|id| self.tracks.iter().find(|s| s.id == id))
    }

    pub fn replace(&mut self, tracks: Vec<Track>, index: usize) {
        let selected = tracks.get(index).map(|t| t.id);
        self.tracks.clear();
        self.append(tracks);
        self.current = selected.or_else(|| self.tracks.first().map(|s| s.id));
        self.rebuild_order();
    }

    pub fn append(&mut self, tracks: Vec<Track>) {
        let mut ids: HashSet<_> = self.tracks.iter().map(|t| t.id).collect();
        let mut added = Vec::new();
        for track in tracks {
            if ids.insert(track.id) {
                added.push(track.id);
                self.tracks.push(track);
            }
        }
        if self.shuffled {
            fastrand::shuffle(&mut added);
        }
        self.order.extend(added);
        if self.current.is_none() {
            self.current = self.order.first().copied();
        }
    }

    pub fn select(&mut self, id: u64) -> bool {
        if self.tracks.iter().any(|s| s.id == id) {
            self.current = Some(id);
            true
        } else {
            false
        }
    }

    pub fn mode(&mut self, repeat: Repeat, shuffled: bool) {
        self.repeat = repeat;
        if self.shuffled != shuffled {
            self.shuffled = shuffled;
            self.rebuild_order();
        }
    }

    fn rebuild_order(&mut self) {
        self.order = self.tracks.iter().map(|s| s.id).collect();
        if self.shuffled {
            self.order.retain(|id| Some(*id) != self.current);
            fastrand::shuffle(&mut self.order);
            if let Some(id) = self.current {
                self.order.insert(0, id);
            }
        }
    }

    pub fn normalize(&mut self) {
        let mut ids = HashSet::new();
        self.tracks.retain(|s| ids.insert(s.id));
        if !self.current.is_some_and(|id| ids.contains(&id)) {
            self.current = self.tracks.first().map(|t| t.id);
        }
        let order_ids: HashSet<_> = self.order.iter().copied().collect();
        if order_ids != ids || self.order.len() != ids.len() || !self.shuffled {
            self.rebuild_order();
        }
    }

    /// Manual Next skips a track even when repeat-one is selected.
    pub fn advance(&mut self, automatic: bool) -> Option<u64> {
        if automatic && self.repeat == Repeat::One {
            return self.current;
        }
        let next = match self
            .current
            .and_then(|id| self.order.iter().position(|x| *x == id))
        {
            Some(i) => self.order.get(i + 1).copied().or_else(|| {
                (self.repeat == Repeat::All)
                    .then(|| self.order.first().copied())
                    .flatten()
            }),
            None => self.order.first().copied(),
        };
        if next.is_some() {
            self.current = next;
        }
        next
    }

    pub fn previous(&mut self) -> Option<u64> {
        let i = self
            .current
            .and_then(|id| self.order.iter().position(|x| *x == id))
            .unwrap_or(0);
        let i = if i == 0 && self.repeat == Repeat::All {
            self.order.len().saturating_sub(1)
        } else {
            i.saturating_sub(1)
        };
        self.current = self.order.get(i).copied();
        self.current
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let active = self.current == Some(id);
        let i = self.order.iter().position(|x| *x == id).unwrap_or(0);
        self.tracks.retain(|t| t.id != id);
        self.order.retain(|x| *x != id);
        if active {
            self.current = self.order.get(i).or_else(|| self.order.last()).copied();
        }
        active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn queue() -> Queue {
        let mut q = Queue::default();
        q.replace(
            (1..=4)
                .map(|id| Track {
                    id,
                    ..Default::default()
                })
                .collect(),
            2,
        );
        q
    }
    #[test]
    fn shuffle_switch_preserves_song() {
        let mut q = queue();
        for _ in 0..100 {
            q.mode(Repeat::All, true);
            assert_eq!(q.current, Some(3));
            q.mode(Repeat::Off, false);
            assert_eq!(q.current, Some(3));
        }
        assert_eq!(q.advance(true), Some(4));
        assert_eq!(q.advance(true), None);
    }
    #[test]
    fn repeat_one_and_manual_skip() {
        let mut q = queue();
        q.mode(Repeat::One, false);
        assert_eq!(q.advance(true), Some(3));
        assert_eq!(q.advance(false), Some(4));
    }
    #[test]
    fn deletion_and_duplicate_append() {
        let mut q = queue();
        q.append(vec![
            Track {
                id: 5,
                ..Default::default()
            };
            2
        ]);
        assert_eq!(q.tracks.len(), 5);
        assert!(!q.remove(1));
        assert_eq!(q.current, Some(3));
        assert!(q.remove(3));
        assert_eq!(q.current, Some(4));
        q.remove(5);
        q.remove(4);
        q.remove(2);
        assert_eq!(q.current(), None);
        assert_eq!(q.advance(true), None);
    }
    #[test]
    fn corrupt_order_is_repaired() {
        let mut q = queue();
        q.order = vec![999, 999];
        q.current = Some(999);
        q.normalize();
        assert_eq!(q.current, Some(1));
        assert_eq!(q.order, vec![1, 2, 3, 4]);
    }
    #[test]
    fn displayed_shuffle_matches_playback_order_and_empty_append() {
        let mut q = Queue::default();
        q.append(vec![Track {
            id: 1,
            ..Default::default()
        }]);
        assert_eq!(q.current, Some(1));
        q.replace(queue().tracks, 2);
        q.mode(Repeat::Off, true);
        let displayed = q.ordered_tracks();
        assert_eq!(displayed[0].id, 3);
        for track in displayed.iter().skip(1) {
            assert_eq!(q.advance(false), Some(track.id));
        }
        assert_eq!(q.advance(false), None);
    }
}
