// The outbound packet modules, decided at the host's serverbound send:
// Blink (`Blink`, outgoing only: every packet is held until Blink turns off
// or "Auto send" reaches "Send threshold"), FakeLag (`FakeLag`: Latency
// holds every packet for Delay ms; Repel holds an attack on a target already
// hurt, and everything after it, until Delay plus up to 100 ms passes or
// the next attack), NoItemRelease (`NoItemRelease`: drops release-use-item
// while in a GUI or holding an allowed item), PotionSaver (`PotionSaver`:
// drops movement packets while standing still with effects on the ground)
// and Regen (`Regen`: 40 extra status packets every fifth tick below the
// health threshold with food above 16).
//
// The host keeps the held packets, in order, each with its due time; this
// state only decides. Blink's Bi-directional mode also holds inbound
// packets (the host's inbound queue) from the moment Blink is on until it
// sends; the reference does it by parking the network thread, which also
// delays keep-alives, where the host here lets connection-critical packets
// through. Repel flushes before the tick once the target's hurt time has
// run down to the expected hit delay (the reference's average attack-to-
// damage delay; its tracker's `initialized` gate is never opened, so its
// flush cannot fire there). Not ported: Blink's fake player and FakeLag's
// 1.7-only Dynamic mode.
use super::super::super::core::settings::{Definition, Kind, Settings};
use super::super::super::inventory::item_limit::{self, HeldItem};
use super::super::render::world::{Line, Point, outline_box};
use serde_json::{Value, json};

pub const BLINK: &str = "blatant.blink";
pub const FAKE_LAG: &str = "world.fake-lag";
pub const NO_ITEM_RELEASE: &str = "combat.no-item-release";
pub const REGEN: &str = "blatant.regen";

pub fn definitions() -> Vec<Definition> {
    let blink = |k: &str| format!("{BLINK}.{k}");
    let lag = |k: &str| format!("{FAKE_LAG}.{k}");
    vec![
        Definition::choice(
            &blink("direction"),
            "Direction",
            &["Outgoing only", "Bi-directional"],
            "Outgoing only",
        ),
        Definition::boolean(&blink("breadcrumbs"), "Breadcrumbs", false),
        Definition::boolean(&blink("auto-send"), "Auto send", false),
        Definition::number(
            &blink("send-threshold"),
            "Send threshold",
            0.0,
            50.0,
            100.0,
            1.0,
        )
        .when(&blink("auto-send"), json!(true)),
        Definition::choice(&lag("mode"), "Mode", &["Latency", "Repel"], "Latency"),
        Definition::number(&lag("delay"), "Delay", 1.0, 100.0, 1000.0, 10.0),
        Definition::new(
            &format!("{NO_ITEM_RELEASE}.allowed-items"),
            "Allowed items",
            Kind::StringList {
                max_entries: 64,
                max_chars: 64,
            },
            json!(["swords", "food", "potions"]),
        ),
        Definition::number(&format!("{REGEN}.health"), "Health", 0.5, 8.0, 10.0, 0.5),
    ]
}

fn number(settings: &Settings, id: &str, default: f64) -> f64 {
    settings.get(id).and_then(Value::as_f64).unwrap_or(default)
}

/// The serverbound packet, as far as these modules tell packets apart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Packet {
    /// Any move-player packet (`C03PacketPlayer`).
    Movement,
    /// An attack on an entity; the target's hurt time if it is living.
    Attack {
        id: i32,
        hurt_time: Option<i32>,
    },
    /// PlayerAction RELEASE_USE_ITEM.
    ReleaseUse,
    Other,
}

/// Which outbound modules are on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Active {
    pub blink: bool,
    pub fake_lag: bool,
    pub no_item_release: bool,
    pub potion_saver: bool,
}

/// The local player at the send.
#[derive(Clone, Debug)]
pub struct Context {
    pub now_ms: u64,
    pub in_gui: bool,
    pub held: HeldItem,
    pub effects: bool,
    /// Forward or strafe input this tick.
    pub moving: bool,
    pub on_ground: bool,
    pub position: [f64; 3],
    /// Degrees.
    pub yaw: f32,
    /// Packets the host is holding.
    pub queued: usize,
    /// 0..100, for Repel's random extra delay.
    pub jitter: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Send,
    Drop,
    /// Hold until the due time (u64::MAX: until flushed).
    Hold {
        due: u64,
    },
}

/// The local player at the tick, for Regen.
#[derive(Clone, Copy, Debug, Default)]
pub struct Vitals {
    pub health: f32,
    pub food: i32,
    pub dead: bool,
    pub on_ground: bool,
    pub jumping: bool,
    pub in_liquid: bool,
    pub riding: bool,
    pub age: u64,
}

#[derive(Debug, Default)]
pub struct Outbound {
    blinking: bool,
    chokes: u32,
    breadcrumbs: Vec<[f64; 3]>,
    repel_deadline: Option<u64>,
    flush: bool,
    saving: bool,
    /// Bi-directional Blink is holding inbound packets.
    inbound_held: bool,
    inbound_flush: bool,
    /// Repel's attack-to-damage delays (ms), the last 20 under 500.
    hit_delays: Vec<u64>,
    last_hit: u64,
    last_attack: u64,
    target: Option<i32>,
}

impl Outbound {
    pub fn reset(&mut self) {
        *self = Self {
            flush: true,
            ..Default::default()
        };
    }
    /// Release everything held (Blink off or its threshold, a Repel attack).
    pub fn take_flush(&mut self) -> bool {
        std::mem::take(&mut self.flush)
    }
    /// Blink's choke count (`getSimpleSuffix`).
    pub fn chokes(&self) -> u32 {
        self.chokes
    }
    /// PotionSaver's GUI colour state.
    pub fn saving(&self) -> bool {
        self.saving
    }
    /// Blink's Direction is Bi-directional.
    pub fn bidirectional(&self, settings: &Settings) -> bool {
        settings
            .get(&format!("{BLINK}.direction"))
            .and_then(Value::as_str)
            == Some("Bi-directional")
    }
    /// Whether Blink just let go of inbound packets it held, once.
    pub fn take_inbound_flush(&mut self) -> bool {
        std::mem::take(&mut self.inbound_flush)
    }
    /// The entity we attacked last, for Repel's hurt-time check.
    pub fn target(&self) -> Option<i32> {
        self.target
    }
    /// The average attack-to-damage delay in ticks (`getExpectedHurtTimeTicks`).
    fn expected_hurt_ticks(&self) -> i32 {
        let count = self.hit_delays.len() as u64;
        (self.hit_delays.iter().sum::<u64>() / count.max(1) / 50) as i32
    }
    /// A damage packet (hurt event or animation) for `id` arrived.
    pub fn hit_confirmed(&mut self, id: i32, now_ms: u64) {
        if self.target != Some(id) {
            return;
        }
        let delay = now_ms.saturating_sub(self.last_hit);
        if delay < 500 {
            self.hit_delays.push(delay);
            if self.hit_delays.len() == 20 {
                self.hit_delays.remove(0);
            }
        }
    }
    fn average_hit_delay(&self) -> u64 {
        self.hit_delays.iter().sum::<u64>() / (self.hit_delays.len() as u64).max(1)
    }
    fn stop_blink(&mut self) {
        if self.blinking || self.inbound_held {
            self.flush |= self.blinking;
            self.inbound_flush |= self.inbound_held;
            self.inbound_held = false;
        }
        self.blinking = false;
        self.chokes = 0;
        self.breadcrumbs.clear();
    }

    pub fn packet(
        &mut self,
        settings: &Settings,
        active: Active,
        packet: Packet,
        ctx: &Context,
    ) -> Verdict {
        if let Packet::Attack { id, hurt_time } = packet {
            self.target = Some(id);
            if hurt_time == Some(0)
                && ctx.now_ms.saturating_sub(self.last_hit) > 400
                && ctx.now_ms.saturating_sub(self.last_attack) > self.average_hit_delay() * 2
            {
                self.last_hit = ctx.now_ms;
            }
            if hurt_time.is_some() {
                self.last_attack = ctx.now_ms;
            }
        }
        if active.no_item_release && packet == Packet::ReleaseUse {
            let allowed: Vec<String> = settings
                .get(&format!("{NO_ITEM_RELEASE}.allowed-items"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            if ctx.in_gui || item_limit::matches(&allowed, &ctx.held) {
                return Verdict::Drop;
            }
        }
        if active.potion_saver {
            self.saving = ctx.effects && !ctx.moving && ctx.on_ground;
            if self.saving && packet == Packet::Movement {
                return Verdict::Drop;
            }
        } else {
            self.saving = false;
        }
        if !active.blink {
            self.stop_blink();
        } else {
            self.blinking = true;
            self.chokes += 1;
            if packet == Packet::Movement
                && ctx.moving
                && self.chokes.is_multiple_of(5)
                && settings
                    .get(&format!("{BLINK}.breadcrumbs"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            {
                let sign = if self.chokes.is_multiple_of(2) {
                    -1.0
                } else {
                    1.0
                };
                let yaw = f64::from(ctx.yaw).to_radians();
                self.breadcrumbs.push([
                    ctx.position[0] + 0.2 * sign * yaw.cos(),
                    ctx.position[1],
                    ctx.position[2] + 0.2 * sign * yaw.sin(),
                ]);
            }
            return Verdict::Hold { due: u64::MAX };
        }
        let delay = number(settings, &format!("{FAKE_LAG}.delay"), 100.0).max(1.0) as u64;
        let repel = settings
            .get(&format!("{FAKE_LAG}.mode"))
            .and_then(Value::as_str)
            == Some("Repel");
        if !active.fake_lag || !repel {
            self.repel_deadline = None;
        }
        if active.fake_lag && !repel {
            return Verdict::Hold {
                due: ctx.now_ms + delay,
            };
        }
        if active.fake_lag {
            if ctx.queued > 0
                && let Some(deadline) = self.repel_deadline
            {
                if matches!(packet, Packet::Attack { .. }) || ctx.now_ms >= deadline {
                    self.flush = true;
                    self.repel_deadline = None;
                    return Verdict::Hold { due: ctx.now_ms };
                }
                return Verdict::Hold { due: deadline };
            }
            if let Packet::Attack {
                hurt_time: Some(hurt),
                ..
            } = packet
            {
                let delay = delay + ctx.jitter % 100;
                if hurt > 0 && i64::from(hurt) <= delay.div_ceil(50) as i64 {
                    let deadline = ctx.now_ms + delay;
                    self.repel_deadline = Some(deadline);
                    return Verdict::Hold { due: deadline };
                }
            }
        }
        if ctx.queued > 0 {
            // Keep order behind packets still held.
            return Verdict::Hold { due: ctx.now_ms };
        }
        Verdict::Send
    }

    /// Per tick: Blink's auto send (and its stop outside a world), then
    /// Repel's pre-tick flush. `target_hurt` is the hurt time of the last
    /// attacked entity, `queued` the packets the host holds.
    pub fn tick(
        &mut self,
        settings: &Settings,
        active: Active,
        in_world: bool,
        target_hurt: Option<i32>,
        queued: usize,
    ) {
        if active.fake_lag
            && queued > 0
            && self.repel_deadline.is_some()
            && target_hurt.is_some_and(|h| h > 0 && h <= self.expected_hurt_ticks())
        {
            self.flush = true;
            self.repel_deadline = None;
        }
        if !active.blink || !in_world {
            self.stop_blink();
            return;
        }
        let bidirectional = self.bidirectional(settings);
        // Switching back to Outgoing only lets inbound packets go.
        self.inbound_flush |= self.inbound_held && !bidirectional;
        self.inbound_held = bidirectional;
        let auto = settings
            .get(&format!("{BLINK}.auto-send"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let threshold = number(settings, &format!("{BLINK}.send-threshold"), 50.0);
        if auto && threshold > 0.0 && f64::from(self.chokes) >= threshold {
            self.stop_blink();
        }
    }

    /// Blink's breadcrumbs: the first yellow, the rest red, 0.2 wide.
    pub fn breadcrumbs(&self, settings: &Settings, out: &mut Vec<Line>) {
        if !self.blinking
            || !settings
                .get(&format!("{BLINK}.breadcrumbs"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            return;
        }
        for (i, at) in self.breadcrumbs.iter().enumerate() {
            let color = if i == 0 { 0xffff_ff00 } else { 0xffff_0000 };
            outline_box(
                out,
                Point([at[0] - 0.1, at[1], at[2] - 0.1]),
                Point([at[0] + 0.1, at[1], at[2] + 0.1]),
                color,
                1.5,
            );
        }
    }
}

/// Regen: how many extra airborne status packets to send this tick.
pub fn regen(settings: &Settings, v: Vitals) -> u32 {
    let threshold = number(settings, &format!("{REGEN}.health"), 8.0) * 2.0;
    let due = f64::from(v.health) < threshold
        && v.food > 16
        && !v.dead
        && v.on_ground
        && !v.jumping
        && !v.in_liquid
        && !v.riding
        && v.age % 5 == 1;
    if due { 40 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(pairs: &[(&str, Value)]) -> Settings {
        let mut s = Settings::default();
        for d in definitions() {
            s.define(d).unwrap();
        }
        for (k, v) in pairs {
            s.set(k, v.clone()).unwrap();
        }
        s
    }

    fn ctx(now_ms: u64, queued: usize) -> Context {
        Context {
            now_ms,
            in_gui: false,
            held: HeldItem::default(),
            effects: false,
            moving: false,
            on_ground: true,
            position: [0.0; 3],
            yaw: 0.0,
            queued,
            jitter: 0,
        }
    }

    const REPEL: Active = Active {
        blink: false,
        fake_lag: true,
        no_item_release: false,
        potion_saver: false,
    };

    #[test]
    fn repel_flushes_before_the_tick_once_hurt_time_reaches_the_expected_delay() {
        let s = settings(&[("world.fake-lag.mode", json!("Repel"))]);
        let mut o = Outbound::default();
        // Two hits confirmed ~150 ms after the attack: expected 3 ticks.
        for at in [1000, 3000] {
            o.packet(
                &s,
                REPEL,
                Packet::Attack {
                    id: 4,
                    hurt_time: Some(0),
                },
                &ctx(at, 0),
            );
            o.hit_confirmed(4, at + 150);
        }
        assert_eq!(o.expected_hurt_ticks(), 3);
        // An attack on a target hurt 2 ticks ago is held until the deadline.
        let verdict = o.packet(
            &s,
            REPEL,
            Packet::Attack {
                id: 4,
                hurt_time: Some(2),
            },
            &ctx(5000, 0),
        );
        assert!(matches!(verdict, Verdict::Hold { due } if due > 5000));
        // Hurt time still above the expected delay: nothing yet.
        o.tick(&s, REPEL, true, Some(5), 1);
        assert!(!o.take_flush());
        // Down to it: the held attack goes before the tick.
        o.tick(&s, REPEL, true, Some(3), 1);
        assert!(o.take_flush());
        // Nothing queued, nothing to flush; and an unhurt target never does.
        o.tick(&s, REPEL, true, Some(3), 0);
        assert!(!o.take_flush());
    }

    #[test]
    fn bidirectional_blink_holds_inbound_until_it_sends() {
        let s = settings(&[("blatant.blink.direction", json!("Bi-directional"))]);
        let blink = Active {
            blink: true,
            fake_lag: false,
            ..REPEL
        };
        let mut o = Outbound::default();
        assert!(o.bidirectional(&s));
        o.tick(&s, blink, true, None, 0);
        assert!(!o.take_inbound_flush(), "holding, nothing to release yet");
        o.tick(&s, Active::default(), true, None, 0);
        assert!(
            o.take_inbound_flush(),
            "turning Blink off releases the inbound queue"
        );
        assert!(!o.take_inbound_flush());
        // Auto send: the threshold sends and the hold starts over.
        let s = settings(&[
            ("blatant.blink.direction", json!("Bi-directional")),
            ("blatant.blink.auto-send", json!(true)),
            ("blatant.blink.send-threshold", json!(2.0)),
        ]);
        o.tick(&s, blink, true, None, 0);
        for _ in 0..2 {
            o.packet(&s, blink, Packet::Other, &ctx(0, 0));
        }
        o.tick(&s, blink, true, None, 0);
        assert!(o.take_inbound_flush() && o.take_flush());
        o.tick(&s, blink, true, None, 0);
        assert!(o.inbound_held);
        // Back to Outgoing only lets go.
        let s = settings(&[]);
        o.tick(&s, blink, true, None, 0);
        assert!(o.take_inbound_flush());
    }
}
