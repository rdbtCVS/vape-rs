// Movement and world-interaction modules ported from the reference's
// blatant/utility packages: Scaffold (Legit), InvWalk and WindCharge. Their
// rotation is the shared one (`core::rotation_runtime`).
pub mod god_bridge;
pub mod inv_walk;
pub mod movement_task;
pub mod scaffold;
pub mod scaffold_bridge;
pub mod step;
pub mod telly_bridge;
pub mod wind_charge;

use super::super::core::settings::Definition;

pub fn definitions() -> Vec<Definition> {
    let mut out = scaffold::definitions();
    out.extend(inv_walk::definitions());
    out.extend(wind_charge::definitions());
    out
}
