// What the modern-item swap modules (HitSwap, ShieldBreaker, AutoMace) see:
// the hotbar with each mace's Density/Breach level, the attacked or nearby
// player's shield (`RotationUtil.q` and the hand's use duration) and the
// reference `AttackKeyController` steps they ask the host for.
use super::super::super::inventory::item_limit::HeldItem;

/// One hotbar stack.
#[derive(Clone, Debug, Default)]
pub struct Slot {
    pub item: HeldItem,
    pub density: i32,
    pub breach: i32,
}
impl Slot {
    fn id(&self) -> Option<&str> {
        self.item.id.as_deref()
    }
    pub fn empty(&self) -> bool {
        self.item.id.is_none()
    }
    /// `AxeItem` (`ItemStackScoreUtil.T` on modern versions: `_axe`).
    pub fn axe(&self) -> bool {
        self.id().is_some_and(|id| id.ends_with("_axe"))
    }
    /// `MaceItem`.
    pub fn mace(&self) -> bool {
        self.id() == Some("mace")
    }
    /// `ItemStackScoreUtil.h`: the id contains `_sword`.
    pub fn sword(&self) -> bool {
        self.id().is_some_and(|id| id.contains("_sword"))
    }
}

/// Another player's shield hand (`RotationUtil.q`: the off hand first).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shield {
    /// That hand is in use (`EntityLivingBase.d(hand) > 0`).
    pub using: bool,
    /// Ticks since the use began (`getUseDuration - d(hand)`).
    pub elapsed: i32,
}
impl Shield {
    /// `RotationUtil.n`: raised, at least one tick into the use.
    pub fn raised(self) -> bool {
        self.using && self.elapsed > 0
    }
    /// The shield blocks once its use is past vanilla's 5-tick warm-up.
    pub fn blocking(self) -> bool {
        self.using && self.elapsed > 5
    }
}

/// Another (non-local) player.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Target {
    /// None when neither hand holds a shield.
    pub shield: Option<Shield>,
}

/// The entity the extended-reach ray trace (Reach and HitBoxes applied) hits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Crosshair {
    /// `Entity.getEntityId`.
    pub id: i32,
    /// It is another player (`EntityOtherPlayerMP`).
    pub player: Option<Target>,
    /// A living entity AutoMace's target filter accepts (filled by the
    /// runtime before the modules run).
    pub mace_target: bool,
    /// `getAttributeValue` of `armor` and `armor_toughness`.
    pub armor: f64,
    pub toughness: f64,
}

#[derive(Clone, Debug, Default)]
pub struct View {
    /// The nine hotbar slots.
    pub hotbar: Vec<Slot>,
    pub selected: usize,
    /// `Minecraft.currentScreen` is set.
    pub screen_open: bool,
    /// The local player is using an item (`isHandActive`).
    pub using_item: bool,
    pub crosshair: Option<Crosshair>,
    /// `RotationUtil.m(player, 5, 90, true)`: the first non-friend player
    /// within 5 blocks and under 90 degrees of yaw.
    pub nearby_player: Option<Target>,
    /// `RotationUtil.u`: falling with at least 1.5 blocks of fall distance.
    pub will_smash: bool,
    pub on_ground: bool,
    /// `posY - prevPosY < 0`.
    pub descending: bool,
    pub fall_distance: f32,
    /// Gliding with an Elytra (`isElytraFlying`).
    pub fall_flying: bool,
    /// `ticksExisted`.
    pub ticks_existed: i32,
    /// `ClientSettings.isInputEnabled`.
    pub input_enabled: bool,
    pub now_ms: u64,
}
impl View {
    /// The crosshair entity, when it is another player.
    pub fn hit_player(&self) -> Option<Target> {
        self.crosshair.and_then(|c| c.player)
    }
    pub fn slot(&self, slot: usize) -> Option<&Slot> {
        self.hotbar.get(slot)
    }
    pub fn held(&self) -> Slot {
        self.slot(self.selected).cloned().unwrap_or_default()
    }
}

/// Who asked `AttackKeyController.requestSyntheticAttack`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    HitSwap,
    ShieldBreaker,
    /// AutoMace, with `isSyntheticAttackInProgress`.
    AutoMace(bool),
    TriggerBot,
    AutoClicker,
    Other,
}

/// A host step, applied in order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Op {
    /// `InventoryPlayer.currentItem = slot`.
    Select(usize),
    /// `releaseAttackKey`: the attack mapping up.
    Release,
    /// `pressAttackKeyNow`: the mapping down and one click.
    Press,
}

/// Selects `slot` and records the step.
pub fn select(view: &mut View, ops: &mut Vec<Op>, slot: usize) {
    view.selected = slot;
    ops.push(Op::Select(slot));
}
