#[derive(Clone, Copy, Debug, Default)]
pub struct CombatSnapshot {
    pub health: f32,
    pub hurt_cooldown: i32,
    pub attacking_entity: bool,
    pub stationary: bool,
    pub nearby_enemy: bool,
    pub looking_at_enemy: bool,
    pub enemy_looking_at_player: bool,
}
#[derive(Debug, Default)]
pub struct CombatGuard {
    blocked: bool,
    remaining: u8,
    stationary_ticks: u8,
    previous_health: Option<f32>,
}
impl CombatGuard {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(&mut self, snapshot: CombatSnapshot) -> bool {
        if self.blocked {
            if self.remaining > 0 {
                self.remaining -= 1;
            } else if snapshot.stationary {
                self.stationary_ticks = self.stationary_ticks.saturating_add(1);
                if self.stationary_ticks >= 5
                    || (!snapshot.looking_at_enemy && !snapshot.enemy_looking_at_player)
                {
                    self.reset();
                    return false;
                }
                self.remaining = 20;
            } else {
                self.stationary_ticks = 0;
                if !snapshot.nearby_enemy {
                    self.reset();
                    return false;
                }
            }
        }
        if self
            .previous_health
            .is_some_and(|health| snapshot.health < health)
            || snapshot.hurt_cooldown == 20
            || snapshot.attacking_entity
        {
            self.blocked = true;
            self.remaining = 20;
        }
        self.previous_health = Some(snapshot.health);
        self.blocked
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn damage_guard_expires_without_threat_and_resets_between_sessions() {
        let mut guard = CombatGuard::default();
        let mut s = CombatSnapshot {
            health: 20.0,
            stationary: true,
            ..CombatSnapshot::default()
        };
        assert!(!guard.update(s));
        s.health = 18.0;
        assert!(guard.update(s));
        for _ in 0..20 {
            assert!(guard.update(s));
        }
        assert!(!guard.update(s));
        s.attacking_entity = true;
        assert!(guard.update(s));
        guard.reset();
        s.attacking_entity = false;
        assert!(!guard.update(s));
    }
}
