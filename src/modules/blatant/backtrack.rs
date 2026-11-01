// Backtrack (`Backtrack`): after we attack an entity, clientbound packets are
// held while that entity moves away from us, so it keeps being drawn (and
// hit) where it just was. The host keeps the held packets in one ordered
// queue with a release time each; this state follows the target's *true*
// position through every packet, including the held ones, and decides.
//
// Per packet, in the reference's order: a pending target (set by our attack)
// becomes the active one and flushes the queue; our own velocity packet or
// a teleport flushes (the host flushes for teleports itself); with no
// target everything flushes; a position update for the target holds when its
// new position is farther from us along the line of sight than the last one
// (`rayIntersectionDistance`), otherwise it flushes; packets that carry no
// position keep the last decision. Held packets are due Latency ms after
// their arrival; one that would be due before the last one held is due just
// after it instead.
//
// Differences: due packets also go at the client tick, not only when the
// next packet arrives; the server-position box is a fixed player size.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::modules::rng::Rng;
use super::super::render::world::{Line, Point, outline_box};
use serde_json::{Value, json};
use std::collections::HashMap;

pub const BACKTRACK: &str = "blatant.backtrack";

pub fn definitions() -> Vec<Definition> {
    let id = |k: &str| format!("{BACKTRACK}.{k}");
    vec![
        Definition::new(
            &id("latency"),
            "Latency",
            Kind::Range {
                min: 0.0,
                max: 500.0,
                step: 1.0,
            },
            json!([50.0, 100.0]),
        ),
        Definition::boolean(&id("render-server-pos"), "Render server pos", true),
        Definition::new(&id("color"), "Color", Kind::Color, json!(0x6405_8669u32))
            .when(&id("render-server-pos"), json!(true)),
    ]
}

/// What a clientbound packet tells Backtrack.
#[derive(Clone, Debug, PartialEq)]
pub enum Packet {
    /// Relative move(s) of an entity; `host` is where the host has it now.
    Moved {
        id: i32,
        host: Option<[f64; 3]>,
        steps: Vec<[i16; 3]>,
    },
    /// Absolute position that counts as a position update (position sync).
    Synced {
        id: i32,
        at: [f64; 3],
    },
    /// Teleport: the state follows, the hold decision does not change.
    Teleported {
        id: i32,
        at: [f64; 3],
    },
    Removed(Vec<i32>),
    /// The local player's velocity packet.
    OwnVelocity,
    Other,
}

#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub now_ms: u64,
    /// The local player's feet.
    pub player: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// Hold until the time; the packet's neighbours keep their order.
    Hold {
        due: u64,
    },
    /// Release everything held for Backtrack, then pass.
    Flush,
}

/// The vanilla 1/4096 quantization the move packets use.
fn apply(base: [f64; 3], delta: [i16; 3]) -> [f64; 3] {
    let mut out = base;
    for i in 0..3 {
        if delta[i] != 0 {
            out[i] = ((base[i] * 4096.0).round() as i64 + i64::from(delta[i])) as f64 / 4096.0;
        }
    }
    out
}

/// Distance from the eye to where its line of sight at `target`'s feet
/// enters the target's expanded box; 0 when it misses.
pub fn ray_distance(player: [f64; 3], target: [f64; 3]) -> f64 {
    let eye = [player[0], player[1] + 1.62, player[2]];
    let toward = [
        target[0] - player[0],
        target[1] - player[1],
        target[2] - player[2],
    ];
    let flat = toward[0].hypot(toward[2]);
    // The reference's rotation maths: yaw from x/z, pitch from the rise.
    let yaw = (-toward[0].atan2(toward[2])).to_degrees();
    let pitch = -(toward[1].atan2(flat)).to_degrees();
    let (sy, cy) = (-yaw.to_radians() - std::f64::consts::PI).sin_cos();
    let (sp, cp) = (-pitch.to_radians()).sin_cos();
    let dir = [sy * -cp, sp, cy * -cp];
    let min = [target[0] - 0.4, target[1] - 0.1, target[2] - 0.4];
    let max = [target[0] + 0.4, target[1] + 1.9, target[2] + 0.4];
    let (mut near, mut far) = (f64::NEG_INFINITY, f64::INFINITY);
    for i in 0..3 {
        if dir[i].abs() < 1e-12 {
            if eye[i] < min[i] || eye[i] > max[i] {
                return 0.0;
            }
            continue;
        }
        let (a, b) = ((min[i] - eye[i]) / dir[i], (max[i] - eye[i]) / dir[i]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    if near > far || far < 0.0 {
        return 0.0;
    }
    near.max(0.0)
}

impl Default for Backtrack {
    fn default() -> Self {
        Self::new(0x51_7cc1_b727_220a)
    }
}

#[derive(Debug)]
pub struct Backtrack {
    random: Rng,
    /// True positions, advanced by every move packet seen.
    states: HashMap<i32, [f64; 3]>,
    active: Option<i32>,
    pending: Option<i32>,
    last: Option<[f64; 3]>,
    delaying: bool,
    last_due: u64,
    held: usize,
    flush: bool,
    render: [[f64; 3]; 2],
    render_target: [f64; 3],
    steps: u32,
    shown: bool,
}

impl Backtrack {
    pub fn new(seed: u64) -> Self {
        Self {
            random: Rng::new(seed),
            states: HashMap::new(),
            active: None,
            pending: None,
            last: None,
            delaying: false,
            last_due: 0,
            held: 0,
            flush: false,
            render: [[0.0; 3]; 2],
            render_target: [0.0; 3],
            steps: 0,
            shown: false,
        }
    }

    /// Disable, world change or disconnect: forget everything.
    pub fn reset(&mut self) {
        let seed = self.random.int(u64::MAX).max(1);
        let flush = true;
        *self = Self {
            flush,
            ..Self::new(seed)
        };
    }

    /// Whether the host should release everything held for Backtrack.
    pub fn take_flush(&mut self) -> bool {
        std::mem::take(&mut self.flush)
    }

    pub fn target(&self) -> Option<i32> {
        self.active
    }

    /// `onPreAttack`: the entity we attack becomes the target at the next
    /// packet.
    pub fn attack(&mut self, id: i32) {
        if self.active != Some(id) && self.pending != Some(id) {
            self.pending = Some(id);
        }
    }

    /// The host's count of packets it holds for Backtrack.
    pub fn held(&mut self, count: usize) {
        self.held = count;
    }

    fn follow(&mut self, packet: &Packet) -> Option<[f64; 3]> {
        match packet {
            Packet::Moved { id, host, steps } => {
                let base = self.states.get(id).copied().or(*host)?;
                let at = steps.iter().fold(base, |at, d| apply(at, *d));
                self.states.insert(*id, at);
                (self.active == Some(*id)).then_some(at)
            }
            Packet::Synced { id, at } => {
                self.states.insert(*id, *at);
                (self.active == Some(*id)).then_some(*at)
            }
            Packet::Teleported { id, at } => {
                self.states.insert(*id, *at);
                None
            }
            Packet::Removed(ids) => {
                for id in ids {
                    self.states.remove(id);
                    if self.active == Some(*id) {
                        self.active = None;
                        self.last = None;
                        self.flush = true;
                    }
                }
                None
            }
            Packet::OwnVelocity | Packet::Other => None,
        }
    }

    /// One clientbound packet (before the host handles it).
    pub fn packet(&mut self, settings: &Settings, ctx: &Context, packet: &Packet) -> Verdict {
        if let Some(id) = self.pending.take() {
            self.active = Some(id);
            self.last = None;
            self.delaying = false;
            if let Some(at) = self.states.get(&id) {
                self.render = [*at; 2];
                self.render_target = *at;
                self.steps = 0;
                self.shown = true;
            }
            self.flush = true;
        }
        let snapshot = self.follow(packet);
        if let Some(at) = snapshot {
            self.render_target = at;
            self.steps = 3;
            self.shown = true;
        }
        if self.take_flush() {
            // A new or removed target: what was held for the old one goes.
            return Verdict::Flush;
        }
        if matches!(packet, Packet::OwnVelocity) {
            return Verdict::Flush;
        }
        if self.active.is_none() {
            return Verdict::Flush;
        }
        if let Some(at) = snapshot {
            self.delaying = self.last.is_some_and(|last| {
                ray_distance(ctx.player, at) > ray_distance(ctx.player, last) + 0.001
            });
            self.last = Some(at);
        }
        if !self.delaying {
            return Verdict::Flush;
        }
        let range = settings
            .get(&format!("{BACKTRACK}.latency"))
            .and_then(Value::as_array)
            .and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)))
            .unwrap_or((50.0, 100.0));
        let wait = self.random.range(range).max(0.0) as u64;
        // `queuePacket`: only a release time before the last one queued
        // moves, to just after it; packets of the same millisecond share one.
        let mut due = ctx.now_ms + wait;
        if self.held > 0 && due < self.last_due {
            due = self.last_due + 1;
        }
        self.last_due = due;
        Verdict::Hold { due }
    }

    /// Client tick: the server-position box glides to its target.
    pub fn tick(&mut self) {
        self.render[0] = self.render[1];
        if self.steps > 0 && self.active.is_some() {
            for i in 0..3 {
                self.render[1][i] +=
                    (self.render_target[i] - self.render[1][i]) / f64::from(self.steps);
            }
            self.steps -= 1;
        }
    }

    /// The server-position box.
    pub fn lines(&self, settings: &Settings, partial: f64, out: &mut Vec<Line>) {
        if self.active.is_none()
            || !self.shown
            || !settings
                .get(&format!("{BACKTRACK}.render-server-pos"))
                .and_then(Value::as_bool)
                .unwrap_or(true)
        {
            return;
        }
        let color = settings
            .get(&format!("{BACKTRACK}.color"))
            .and_then(Value::as_u64)
            .unwrap_or(0x6405_8669) as u32;
        let at = |i: usize| self.render[0][i] + (self.render[1][i] - self.render[0][i]) * partial;
        outline_box(
            out,
            Point([at(0) - 0.3, at(1) + 0.01, at(2) - 0.3]),
            Point([at(0) + 0.3, at(1) + 1.81, at(2) + 0.3]),
            color | 0xff00_0000,
            1.5,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        s
    }

    fn ctx(now_ms: u64) -> Context {
        Context {
            now_ms,
            player: [0.0, 0.0, 0.0],
        }
    }

    fn step(id: i32, dz: i16) -> Packet {
        Packet::Moved {
            id,
            host: Some([0.0, 0.0, 3.0]),
            steps: vec![[0, 0, dz]],
        }
    }

    #[test]
    fn a_burst_in_one_millisecond_shares_its_release_time() {
        // 300 pushing cows send thousands of packets a second: their release
        // times must not run ahead of the clock (`queuePacket` only moves a
        // time that is before the last one).
        let mut s = settings();
        s.set(&format!("{BACKTRACK}.latency"), json!([500.0, 500.0]))
            .unwrap();
        let mut b = Backtrack::new(7);
        b.attack(5);
        b.packet(&s, &ctx(0), &Packet::Other);
        b.packet(&s, &ctx(10), &step(5, 0));
        assert!(matches!(
            b.packet(&s, &ctx(20), &step(5, 2000)),
            Verdict::Hold { due: 520 }
        ));
        b.held(1);
        for i in 0..1000 {
            assert_eq!(
                b.packet(&s, &ctx(20), &Packet::Other),
                Verdict::Hold { due: 520 }
            );
            b.held(i + 2);
        }
        assert_eq!(
            b.packet(&s, &ctx(21), &Packet::Other),
            Verdict::Hold { due: 521 }
        );
    }

    #[test]
    fn farther_target_positions_are_held_in_order_and_nearer_ones_flush() {
        let s = settings();
        let mut b = Backtrack::new(7);
        b.attack(5);
        // The attack's target is promoted at the next packet, which flushes.
        assert_eq!(b.packet(&s, &ctx(0), &Packet::Other), Verdict::Flush);
        assert_eq!(b.packet(&s, &ctx(10), &step(5, 0)), Verdict::Flush);
        let mut dues = vec![];
        for (i, now) in [20u64, 21, 22].into_iter().enumerate() {
            // Walking away along +z.
            let verdict = b.packet(&s, &ctx(now), &step(5, 2000 * (i as i16 + 1)));
            let Verdict::Hold { due } = verdict else {
                panic!("{verdict:?}")
            };
            dues.push(due);
            b.held(i + 1);
        }
        assert!(dues.windows(2).all(|w| w[0] <= w[1]), "{dues:?}");
        assert!(dues.iter().all(|d| (70..=122).contains(d)), "{dues:?}");
        // A packet without a position keeps the decision: held behind them.
        assert!(matches!(
            b.packet(&s, &ctx(30), &Packet::Other),
            Verdict::Hold { .. }
        ));
        // Walking back toward us flushes everything.
        assert_eq!(b.packet(&s, &ctx(40), &step(5, -9000)), Verdict::Flush);
    }

    #[test]
    fn own_velocity_removal_and_reset_flush() {
        let s = settings();
        let mut b = Backtrack::new(3);
        assert_eq!(
            b.packet(&s, &ctx(0), &Packet::Other),
            Verdict::Flush,
            "no target"
        );
        b.attack(9);
        b.packet(&s, &ctx(0), &step(9, 0));
        b.packet(&s, &ctx(1), &step(9, 3000));
        assert!(matches!(
            b.packet(&s, &ctx(2), &step(9, 3000)),
            Verdict::Hold { .. }
        ));
        b.held(1);
        assert_eq!(b.packet(&s, &ctx(3), &Packet::OwnVelocity), Verdict::Flush);
        assert!(matches!(
            b.packet(&s, &ctx(4), &step(9, 3000)),
            Verdict::Hold { .. }
        ));
        assert_eq!(
            b.packet(&s, &ctx(5), &Packet::Removed(vec![9])),
            Verdict::Flush
        );
        assert_eq!(b.target(), None);
        b.attack(9);
        b.packet(&s, &ctx(6), &Packet::Other);
        b.held(2);
        b.reset();
        assert!(b.take_flush(), "reset asks for what is held to go");
        assert_eq!(b.target(), None);
    }

    #[test]
    fn ray_distance_grows_as_the_target_backs_off() {
        let near = ray_distance([0.0; 3], [0.0, 0.0, 2.0]);
        let far = ray_distance([0.0; 3], [0.0, 0.0, 4.0]);
        assert!((near - 1.6).abs() < 0.2, "{near}");
        assert!(far > near + 1.5, "{near} {far}");
        // Moved packets use the 1/4096 grid and keep zero axes exact.
        assert_eq!(apply([1.5, 2.0, 3.0], [4096, 0, -4096]), [2.5, 2.0, 2.0]);
    }
}
