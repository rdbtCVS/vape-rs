// Movement and world-interaction modules ported from the reference's
// blatant/utility packages: Scaffold (Legit), InvWalk and WindCharge, plus an
// isolated port of the reference's fixed rotation controller.
pub mod inv_walk;
pub mod rotation;
pub mod scaffold;
pub mod step;
pub mod wind_charge;

use super::super::core::settings::Definition;

pub fn definitions() -> Vec<Definition> {
    let mut out = scaffold::definitions();
    out.extend(inv_walk::definitions());
    out.extend(wind_charge::definitions());
    out
}
