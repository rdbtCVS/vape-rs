use super::{
    Snapshot,
    hud::{ArmorPiece, PotionView},
};
use std::{
    collections::{BTreeMap, VecDeque},
    time::{Duration, Instant},
};
#[derive(Debug, Default)]
pub struct HudState {
    clicks: [VecDeque<Instant>; 2],
    low_armor: [bool; 4],
    broken_until: [Option<Instant>; 4],
    effect_max: BTreeMap<String, i32>,
}
impl HudState {
    pub fn reset_session(&mut self) {
        self.low_armor = [false; 4];
        self.broken_until = [None; 4];
        self.effect_max.clear();
        self.clicks.iter_mut().for_each(VecDeque::clear);
    }
    pub fn click(&mut self, button: usize) {
        let Some(queue) = self.clicks.get_mut(button) else {
            return;
        };
        let now = Instant::now();
        while queue
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(1))
        {
            queue.pop_front();
        }
        if queue.len() == 4096 {
            queue.pop_front();
        }
        queue.push_back(now);
    }
    pub fn cps(&self) -> [u16; 2] {
        let now = Instant::now();
        self.clicks.each_ref().map(|q| {
            q.iter()
                .filter(|t| now.duration_since(**t) < Duration::from_secs(1))
                .count() as u16
        })
    }
    pub fn update(&mut self, snapshot: &Snapshot) {
        let now = Instant::now();
        for i in 0..4 {
            if let Some(piece) = &snapshot.armor[i] {
                self.low_armor[i] = piece.percent().is_some_and(|v| v <= 2);
                self.broken_until[i] = None;
            } else if self.low_armor[i] {
                self.broken_until[i] = Some(now + Duration::from_secs(4));
                self.low_armor[i] = false;
            }
        }
        self.effect_max
            .retain(|id, _| snapshot.effects.iter().any(|e| &e.id == id));
        for effect in &snapshot.effects {
            self.effect_max
                .entry(effect.id.clone())
                .and_modify(|max| *max = (*max).max(effect.duration))
                .or_insert(effect.duration);
        }
    }
    pub fn armor(&self, snapshot: &Snapshot, preview: bool) -> Vec<ArmorPiece> {
        if preview && snapshot.armor.iter().all(Option::is_none) {
            return (0..4)
                .map(|slot| ArmorPiece {
                    slot,
                    max_damage: 100,
                    damage: 0,
                    preview: true,
                    broken: false,
                })
                .collect();
        }
        let now = Instant::now();
        (0..4)
            .filter_map(|slot| {
                snapshot.armor[slot].clone().or_else(|| {
                    self.broken_until[slot]
                        .filter(|until| *until > now)
                        .map(|_| ArmorPiece {
                            slot,
                            max_damage: 100,
                            damage: 100,
                            preview: false,
                            broken: true,
                        })
                })
            })
            .collect()
    }
    pub fn effects(&self, snapshot: &Snapshot, preview: bool) -> Vec<PotionView> {
        let mut effects = snapshot.effects.clone();
        if effects.is_empty() && preview {
            effects = vec![
                PotionView::preview("speed", 6500, false),
                PotionView::preview("slowness", 5000, true),
                PotionView::preview("fire_resistance", 1000, false),
            ];
        }
        for effect in &mut effects {
            effect.maximum = self
                .effect_max
                .get(&effect.id)
                .copied()
                .unwrap_or(if preview { 10000 } else { effect.duration });
        }
        effects
    }
}
