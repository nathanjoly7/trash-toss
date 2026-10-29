//! Test automatique (feature `autoplay`) : ramasse le déchet le plus proche et le lance en cloche
//! vers la poubelle, en passant par la vraie main, le vrai comptage et la vraie fin de partie.
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{BIN_POS, Hand, Player, Trash};

const GRAVITY: f32 = 9.81;
const FLIGHT_SECS: f32 = 1.1;

pub fn autoplay(
    time: Res<Time>,
    mut next_action: Local<f32>,
    mut hand: ResMut<Hand>,
    camera: Single<&Transform, With<Player>>,
    mut trash: Query<
        (
            Entity,
            &mut Position,
            &mut LinearVelocity,
            &mut GravityScale,
            &LinearDamping,
        ),
        With<Trash>,
    >,
) {
    let now = time.elapsed_secs();
    if now < 2.0 || now < *next_action {
        return;
    }
    match hand.held {
        None => {
            let nearest = trash
                .iter()
                .min_by(|a, b| {
                    let da = a.1.0.distance(camera.translation);
                    let db = b.1.0.distance(camera.translation);
                    da.total_cmp(&db)
                })
                .map(|item| item.0);
            if let Some(entity) = nearest
                && let Ok((_, mut position, mut velocity, mut gravity, _)) = trash.get_mut(entity)
            {
                position.0 = camera.translation + camera.forward() * 0.75;
                velocity.0 = Vec3::ZERO;
                gravity.0 = 0.0;
                hand.held = Some(entity);
                hand.armed = true;
                *next_action = now + 0.6;
            }
        }
        Some(entity) => {
            if let Ok((_, position, mut velocity, mut gravity, damping)) = trash.get_mut(entity) {
                // Tir balistique avec amortissement linéaire : x(T) = v0 (1 - e^(-cT)) / c.
                let c = damping.0.max(1e-3);
                let k = (1.0 - (-c * FLIGHT_SECS).exp()) / c;
                let to_bin = BIN_POS + Vec3::Y * 0.45 - position.0;
                let mut v = to_bin / k;
                v.y = (to_bin.y + GRAVITY * FLIGHT_SECS / c) / k - GRAVITY / c;
                velocity.0 = v;
                gravity.0 = 1.0;
            }
            *hand = Hand::default();
            // Laisser atterrir avant de reprendre un déchet : sinon on le rattrape en plein vol.
            *next_action = now + FLIGHT_SECS + 0.8;
        }
    }
}
