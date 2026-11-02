//! Test automatique (feature `autoplay`) : joue la partie tout seul, en passant par la vraie main,
//! le vrai comptage et la vraie fin de partie.
//! - Trash Toss : ramasse le déchet le plus proche et le lance en cloche vers la poubelle.
//! - Bureau propre : passe par les vrais choix du menu et la vraie armoire, avec deux erreurs
//!   volontaires (brancher la clé USB, ranger le post-it dans l'armoire) : 5 / 5, 1400 points.
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    BIN_POS, ContextMenu, Hand, MenuChoices, Player, Taps, Trash,
    clean_desk::{Armoire, Destination, Outcome, Receptacle, Risk},
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

/// Une étape du scénario de test : (pause avant l'étape suivante, action).
enum Step {
    Choose(&'static str, fn(&Outcome) -> bool),
    PutInArmoire,
    ThrowInShredder,
}

const SCRIPT: [(f32, Step); 9] = [
    (1.5, Step::Choose("Clé USB trouvée par terre", |o| matches!(o, Outcome::Wrong(_)))),
    (1.5, Step::Choose("Clé USB trouvée par terre", |o| matches!(o, Outcome::Right(..)))),
    (1.5, Step::Choose("Session ouverte", |o| matches!(o, Outcome::Right(..)))),
    (1.5, Step::Choose("Tableau blanc du projet", |o| matches!(o, Outcome::Right(..)))),
    (1.5, Step::Choose("Post-it avec un mot de passe", |o| matches!(o, Outcome::Take))),
    (1.0, Step::PutInArmoire),
    (FLIGHT_SECS + 1.2, Step::ThrowInShredder),
    (1.8, Step::Choose("Contrat client signé, confidentiel", |o| matches!(o, Outcome::Take))),
    (1.5, Step::PutInArmoire),
];

pub fn autoplay_clean_desk(
    time: Res<Time>,
    mut state: Local<(f32, usize)>,
    mut menu: ResMut<ContextMenu>,
    mut hand: ResMut<Hand>,
    mut taps: ResMut<Taps>,
    receptacles: Query<(&Transform, &Receptacle), Without<Player>>,
    risks: Query<(Entity, &Risk, &MenuChoices)>,
    armoire: Query<Entity, With<Armoire>>,
    mut bodies: Query<(&Position, &mut LinearVelocity, &mut GravityScale, &LinearDamping)>,
) {
    let (next_action, step) = &mut *state;
    let now = time.elapsed_secs();
    if now < 2.0 || now < *next_action || armoire.is_empty() {
        return;
    }
    let Some((pause, action)) = SCRIPT.get(*step) else { return };
    match action {
        Step::Choose(title, wanted) => {
            if let Some((entity, risk, _)) = risks.iter().find(|(_, _, choices)| choices.title == *title)
                && let Some(index) = risk.outcomes.iter().position(wanted)
            {
                menu.chosen = Some((entity, index));
            }
        }
        Step::PutInArmoire => taps.0.extend(armoire.iter()),
        Step::ThrowInShredder => {
            let shredder = receptacles.iter().find(|(_, r)| r.destination == Destination::Shredder);
            if let Some(held) = hand.held
                && let Some((zone, _)) = shredder
                && let Ok((position, mut velocity, mut gravity, damping)) = bodies.get_mut(held)
            {
                velocity.0 = ballistic(position.0, zone.translation, damping.0);
                gravity.0 = 1.0;
                *hand = Hand::default();
            }
        }
    }
    *step += 1;
    *next_action = now + pause;
}
