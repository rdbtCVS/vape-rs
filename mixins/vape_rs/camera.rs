use horizon_client::camera::{BobState, bob_matrix};

/// Operate on a copied render snapshot. Health and physical input stay owned by
/// the host; no restoration timer or temporary mutation can escape this call.
pub(super) fn without_hurt(mut state: BobState, tilt: f64, bob_view: bool) -> glam::Mat4 {
    if let Some(living) = &mut state.living {
        living.hurt_time = -1.0;
    }
    glam::Mat4::from_cols_array(&bob_matrix(&state, tilt, bob_view).m)
}
#[cfg(test)]
mod tests {
    use super::*;
    use horizon_client::camera::HurtBob;
    #[test]
    fn damage_shake_is_removed_and_view_bob_and_input_snapshot_are_preserved() {
        let state = BobState {
            living: Some(HurtBob {
                hurt_time: 5.0,
                hurt_duration: 10,
                hurt_dir: 27.0,
                death_time: None,
            }),
            player: Some((0.7, 0.1)),
        };
        let expected = glam::Mat4::from_cols_array(
            &bob_matrix(
                &BobState {
                    living: None,
                    player: state.player,
                },
                1.0,
                true,
            )
            .m,
        );
        assert_eq!(without_hurt(state, 1.0, true), expected);
        assert_ne!(expected, glam::Mat4::IDENTITY);
        assert_eq!(state.living.unwrap().hurt_time, 5.0);
        let original = glam::Mat4::from_cols_array(&bob_matrix(&state, 1.0, true).m);
        assert_ne!(original, expected);
    }
    #[test]
    fn death_camera_effect_remains() {
        let state = BobState {
            living: Some(HurtBob {
                hurt_time: 5.0,
                hurt_duration: 10,
                hurt_dir: 27.0,
                death_time: Some(12.0),
            }),
            player: None,
        };
        assert_ne!(without_hurt(state, 1.0, false), glam::Mat4::IDENTITY);
    }
}
