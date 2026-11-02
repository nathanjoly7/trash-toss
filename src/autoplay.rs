//! Test automatique (feature `autoplay`) : joue la partie tout seul, en passant par la vraie main,
//! le vrai comptage et la vraie fin de partie.
//! - Trash Toss : ramasse le déchet le plus proche et le lance en cloche vers la poubelle.
//! - Bureau propre : vise d'abord exprès le mauvais bac avec le contrat (l'erreur doit être comptée
//!   et l'objet renvoyé), puis trie chaque objet dans le bon bac et touche les deux cibles.
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    BIN_POS, Hand, Player, Taps, Touchable, Trash,
    clean_desk::{Action, Destination, Receptacle, Risk},
};

const GRAVITY: f32 = 9.81;
const FLIGHT_SECS: f32 = 1.1;

/// Tir balistique avec amortissement linéaire : x(T) = v0 (1 - e^(-cT)) / c.
fn ballistic(from: Vec3, to: Vec3, damping: f32) -> Vec3 {
    let c = damping.max(1e-3);
    let k = (1.0 - (-c * FLIGHT_SECS).exp()) / c;
    let delta = to - from;
    let mut v = delta / k;
    v.y = (delta.y + GRAVITY * FLIGHT_SECS / c) / k - GRAVITY / c;
    v
}

fn hold_point(camera: &Transform) -> Vec3 {
    camera.translation + camera.forward() * 0.75
}

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
                position.0 = hold_point(&camera);
                velocity.0 = Vec3::ZERO;
                gravity.0 = 0.0;
                hand.held = Some(entity);
                hand.armed = true;
                *next_action = now + 0.6;
            }
        }
        Some(entity) => {
            if let Ok((_, position, mut velocity, mut gravity, damping)) = trash.get_mut(entity) {
                velocity.0 = ballistic(position.0, BIN_POS + Vec3::Y * 0.45, damping.0);
                gravity.0 = 1.0;
            }
            *hand = Hand::default();
            // Laisser atterrir avant de reprendre un déchet : sinon on le rattrape en plein vol.
            *next_action = now + FLIGHT_SECS + 0.8;
        }
    }
}

pub fn autoplay_clean_desk(
    time: Res<Time>,
    mut state: Local<(f32, bool)>,
    mut hand: ResMut<Hand>,
    mut taps: ResMut<Taps>,
    camera: Single<&Transform, With<Player>>,
    receptacles: Query<(&Transform, &Receptacle), Without<Player>>,
    mut items: Query<
        (
            Entity,
            &Risk,
            &mut Position,
            &mut LinearVelocity,
            &mut GravityScale,
            &LinearDamping,
        ),
    >,
    targets: Query<Entity, (With<Risk>, With<Touchable>)>,
) {
    let (next_action, wrong_done) = &mut *state;
    let now = time.elapsed_secs();
    if now < 2.0 || now < *next_action {
        return;
    }
    match hand.held {
        None => {
            let first = items
                .iter()
                .find(|item| matches!(item.1.action, Action::Bring(_)))
                .map(|item| item.0);
            if let Some(entity) = first
                && let Ok((_, _, mut position, mut velocity, mut gravity, _)) = items.get_mut(entity)
            {
                position.0 = hold_point(&camera);
                velocity.0 = Vec3::ZERO;
                gravity.0 = 0.0;
                hand.held = Some(entity);
                hand.armed = true;
                *next_action = now + 0.6;
            } else if let Some(target) = targets.iter().next() {
                taps.0.push(target);
                *next_action = now + 1.5;
            }
        }
        Some(entity) => {
            if let Ok((_, risk, position, mut velocity, mut gravity, damping)) = items.get_mut(entity)
                && let Action::Bring(expected) = risk.action
            {
                let aim = if expected == Destination::Cabinet && !*wrong_done {
                    *wrong_done = true;
                    Destination::Shredder
                } else {
                    expected
                };
                if let Some((zone, _)) = receptacles.iter().find(|(_, r)| r.destination == aim) {
                    velocity.0 = ballistic(position.0, zone.translation, damping.0);
                }
                gravity.0 = 1.0;
            }
            *hand = Hand::default();
            // Un objet renvoyé par le mauvais bac a besoin de plus de temps pour retomber.
            *next_action = now + FLIGHT_SECS + 1.6;
        }
    }
}
