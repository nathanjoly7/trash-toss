//! Mode « Bureau propre » : 18 h, on part en dernier et on sécurise cinq risques cyber et RGPD.
//! Trois objets se trient dans le bon bac (broyeuse, armoire fermée à clé, service informatique),
//! deux cibles se touchent (session ouverte à verrouiller, tableau blanc à effacer).
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    Game, Grabbable, Hand, MessageText, Obstacle, Player, RoundEntity, ScoreText, Taps, Touchable,
    USE_POINTER_LOCK,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Destination {
    Shredder,
    Cabinet,
    ItDesk,
}

/// Zone de dépôt : une boîte de détection centrée sur le Transform de l'entité.
#[derive(Component)]
pub(crate) struct Receptacle {
    pub(crate) destination: Destination,
    pub(crate) half_extents: Vec3,
}

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Bring(Destination),
    Touch(TouchEffect),
}

#[derive(Clone, Copy)]
pub(crate) enum TouchEffect {
    LockScreen,
    EraseBoard,
}

#[derive(Component)]
pub(crate) struct Risk {
    pub(crate) label: &'static str,
    pub(crate) action: Action,
    success: &'static str,
    /// Dit pourquoi le bac choisi est le mauvais (objets à trier seulement).
    wrong: &'static str,
}

/// Renvoyé par un mauvais bac : on ne le reteste pas pendant qu'il en ressort.
#[derive(Component)]
pub(crate) struct Rejected(f32);

#[derive(Component)]
pub(crate) struct OpenSession {
    locked: Handle<StandardMaterial>,
}

#[derive(Component)]
pub(crate) struct DesktopWindow;

#[derive(Component)]
pub(crate) struct BoardInk;

/// Étiquette d'interface qui suit un point de la scène.
#[derive(Component)]
pub(crate) struct WorldLabel(Vec3);

#[derive(Component)]
pub(crate) struct HeldText;

#[derive(Component)]
pub(crate) struct DebriefPanel;

#[derive(Component)]
pub(crate) struct DebriefResult;

const LABEL_WIDTH: f32 = 180.0;

const RULES: &str = "\u{2022} Mot de passe écrit : à la broyeuse, puis on le change.\n\
\u{2022} Session : verrouillée dès qu'on quitte son poste (Windows + L).\n\
\u{2022} Document confidentiel : rangé sous clé, jamais laissé à l'imprimante.\n\
\u{2022} Clé USB inconnue : jamais branchée, remise au service informatique.\n\
\u{2022} Tableau blanc : effacé à la fin de la réunion.";

pub(crate) fn points(errors: u32) -> u32 {
    2000u32.saturating_sub(300 * errors).max(500)
}

// ---------- Décor fixe : bacs, imprimante, étiquettes ----------

fn spawn_static_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    size: Vec3,
    position: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(position),
        RigidBody::Static,
        Collider::cuboid(size.x, size.y, size.z),
    ));
}

/// Bac ouvert : un fond et quatre bords, posé avec son fond à la hauteur `floor_y`.
fn spawn_tray(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    center: Vec3,
    size: Vec2,
    wall: f32,
    destination: Destination,
) {
    let t = 0.02;
    let (hx, hz) = (size.x / 2.0, size.y / 2.0);
    spawn_static_box(commands, meshes, material, Vec3::new(size.x, t, size.y), center + Vec3::Y * (t / 2.0));
    for (half, offset) in [
        (Vec3::new(hx, wall / 2.0, t / 2.0), Vec3::new(0.0, 0.0, hz)),
        (Vec3::new(hx, wall / 2.0, t / 2.0), Vec3::new(0.0, 0.0, -hz)),
        (Vec3::new(t / 2.0, wall / 2.0, hz), Vec3::new(hx, 0.0, 0.0)),
        (Vec3::new(t / 2.0, wall / 2.0, hz), Vec3::new(-hx, 0.0, 0.0)),
    ] {
        spawn_static_box(commands, meshes, material, half * 2.0, center + offset + Vec3::Y * (wall / 2.0));
    }
    commands.spawn((
        Transform::from_translation(center + Vec3::Y * (wall / 2.0 + 0.02)),
        Receptacle {
            destination,
            half_extents: Vec3::new(hx, wall / 2.0 + 0.05, hz),
        },
    ));
}

pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut game: ResMut<Game>,
    time: Res<Time>,
) {
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.17, 0.19),
        perceptual_roughness: 0.5,
        ..default()
    });
    let steel = materials.add(StandardMaterial {
        base_color: Color::srgb(0.62, 0.64, 0.67),
        metallic: 0.5,
        perceptual_roughness: 0.4,
        ..default()
    });
    let white = materials.add(Color::srgb(0.9, 0.9, 0.88));
    let it_blue = materials.add(Color::srgb(0.12, 0.38, 0.85));
    let brass = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.7, 0.2),
        metallic: 0.9,
        perceptual_roughness: 0.3,
        ..default()
    });

    // Broyeuse : bloc sombre, fente sur le dessus, bac peu profond pour retenir ce qu'on y lance.
    let shredder = Vec3::new(-1.2, 0.0, 3.8);
    spawn_static_box(&mut commands, &mut meshes, &dark, Vec3::new(0.45, 0.6, 0.32), shredder + Vec3::Y * 0.3);
    spawn_tray(&mut commands, &mut meshes, &dark, shredder + Vec3::Y * 0.6, Vec2::new(0.45, 0.32), 0.06, Destination::Shredder);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.32, 0.005, 0.03))),
        MeshMaterial3d(materials.add(Color::BLACK)),
        Transform::from_translation(shredder + Vec3::Y * 0.623),
    ));

    // Armoire fermée à clé : caisson métallique, serrure dorée, tiroir ouvert sur le dessus.
    let cabinet = Vec3::new(0.0, 0.0, 3.6);
    spawn_static_box(&mut commands, &mut meshes, &steel, Vec3::new(0.8, 0.72, 0.45), cabinet + Vec3::Y * 0.36);
    spawn_tray(&mut commands, &mut meshes, &steel, cabinet + Vec3::Y * 0.72, Vec2::new(0.7, 0.42), 0.14, Destination::Cabinet);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.07, 0.09, 0.02))),
        MeshMaterial3d(brass),
        Transform::from_translation(cabinet + Vec3::new(0.0, 0.5, 0.235)),
    ));

    // Service informatique : bac bleu sur un meuble blanc.
    let it_desk = Vec3::new(1.2, 0.0, 3.8);
    spawn_static_box(&mut commands, &mut meshes, &white, Vec3::new(0.5, 0.6, 0.36), it_desk + Vec3::Y * 0.3);
    spawn_tray(&mut commands, &mut meshes, &it_blue, it_desk + Vec3::Y * 0.6, Vec2::new(0.46, 0.34), 0.1, Destination::ItDesk);

    // Imprimante contre le mur du fond, derrière le joueur.
    let printer = Vec3::new(3.6, 0.0, 7.55);
    spawn_static_box(&mut commands, &mut meshes, &steel, Vec3::new(0.9, 0.72, 0.5), printer + Vec3::Y * 0.36);
    spawn_static_box(&mut commands, &mut meshes, &white, Vec3::new(0.55, 0.3, 0.42), printer + Vec3::Y * 0.87);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.4, 0.004, 0.12))),
        MeshMaterial3d(dark.clone()),
        Transform::from_translation(printer + Vec3::new(0.0, 1.022, 0.12)),
    ));

    // On contourne les meubles : le joueur est repoussé à cette distance de leur centre.
    for (center, radius) in [(shredder, 0.45), (cabinet, 0.6), (it_desk, 0.45), (printer, 0.75)] {
        commands.spawn((Transform::from_translation(center), Obstacle { radius }));
    }

    for (text, anchor) in [
        ("Broyeuse", shredder + Vec3::Y * 1.0),
        ("Armoire fermée à clé", cabinet + Vec3::Y * 1.15),
        ("Service informatique", it_desk + Vec3::Y * 1.0),
    ] {
        commands
            .spawn((
                WorldLabel(anchor),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(LABEL_WIDTH),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Visibility::Hidden,
            ))
            .with_children(|label| {
                label.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    children![(
                        Text::new(text),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                    )],
                ));
            });
    }

    spawn_ui(&mut commands);
    spawn_round(&mut commands, &mut meshes, &mut materials, &mut game, time.elapsed_secs());
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Px(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                HeldText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextShadow::default(),
            ));
        });

    commands
        .spawn((
            DebriefPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(5.0),
                right: Val::Percent(5.0),
                top: Val::Percent(14.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(20.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.09, 0.13, 0.92)),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("Open space sécurisé"),
                TextFont {
                    font_size: FontSize::Px(26.0),
                    ..default()
                },
            ));
            panel.spawn((
                DebriefResult,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(19.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.2)),
            ));
            panel.spawn((
                Text::new(RULES),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
            ));
            panel.spawn((
                Text::new(if USE_POINTER_LOCK {
                    "Clic ou R pour rejouer"
                } else {
                    "Touchez l'écran pour continuer"
                }),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.65)),
            ));
        });
}

// ---------- Une manche : les cinq risques ----------

pub(crate) fn spawn_round(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    game: &mut Game,
    now: f32,
) {
    let paper = materials.add(Color::srgb(0.97, 0.97, 0.95));
    let ink = materials.add(Color::srgb(0.35, 0.36, 0.4));
    let red = materials.add(Color::srgb(0.85, 0.12, 0.12));

    // 1. Post-it avec le mot de passe, collé sur l'écran du bureau de gauche (sans gravité : il tient).
    commands.spawn((
        RoundEntity,
        Grabbable,
        Risk {
            label: "Post-it avec un mot de passe",
            action: Action::Bring(Destination::Shredder),
            success: "Mot de passe détruit. Pensez aussi à le changer.",
            wrong: "Un mot de passe écrit ne se garde pas : à la broyeuse, puis on le change.",
        },
        Mesh3d(meshes.add(Cuboid::new(0.006, 0.1, 0.1))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.88, 0.2),
            emissive: LinearRgba::rgb(0.25, 0.2, 0.02),
            ..default()
        })),
        Transform::from_xyz(-5.722, 1.24, 2.17),
        item_physics(Collider::cuboid(0.006, 0.1, 0.1), 0.6),
        GravityScale(0.0),
    ));

    // 2. Contrat signé oublié sur l'imprimante : feuille blanche, bandeau rouge « confidentiel ».
    commands
        .spawn((
            RoundEntity,
            Grabbable,
            Risk {
                label: "Contrat client signé, confidentiel",
                action: Action::Bring(Destination::Cabinet),
                success: "Contrat rangé sous clé.",
                wrong: "Un contrat signé est un original : il se range sous clé, il ne se détruit pas.",
            },
            Mesh3d(meshes.add(Cuboid::new(0.21, 0.006, 0.297))),
            MeshMaterial3d(paper),
            Transform::from_xyz(3.6, 1.03, 7.45).with_rotation(Quat::from_rotation_y(0.2)),
            item_physics(Collider::cuboid(0.21, 0.006, 0.297), 0.6),
            GravityScale(1.0),
        ))
        .with_children(|sheet| {
            sheet.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.002, 0.035))),
                MeshMaterial3d(red.clone()),
                Transform::from_xyz(0.0, 0.004, -0.1),
            ));
            for z in [-0.04, -0.01, 0.02, 0.05, 0.08] {
                sheet.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.16, 0.002, 0.008))),
                    MeshMaterial3d(ink.clone()),
                    Transform::from_xyz(0.0, 0.004, z),
                ));
            }
        });

    // 3. Clé USB trouvée par terre.
    commands
        .spawn((
            RoundEntity,
            Grabbable,
            Risk {
                label: "Clé USB trouvée par terre",
                action: Action::Bring(Destination::ItDesk),
                success: "Clé remise au service informatique, sans la brancher.",
                wrong: "Une clé inconnue peut contenir un logiciel malveillant : on la remet au service informatique.",
            },
            Mesh3d(meshes.add(Cuboid::new(0.075, 0.016, 0.026))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.35, 0.08),
                emissive: LinearRgba::rgb(0.15, 0.04, 0.0),
                ..default()
            })),
            Transform::from_xyz(-2.0, 0.02, 5.3).with_rotation(Quat::from_rotation_y(0.7)),
            item_physics(Collider::cuboid(0.075, 0.016, 0.026), 0.1),
            GravityScale(1.0),
        ))
        .with_children(|key| {
            key.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.022, 0.01, 0.018))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.8, 0.82),
                    metallic: 1.0,
                    perceptual_roughness: 0.2,
                    ..default()
                })),
                Transform::from_xyz(0.048, 0.0, 0.0),
            ));
        });

    // 4. Session restée ouverte sur l'écran du bureau de droite : bureau Windows lumineux.
    let screen = Vec3::new(5.728, 1.13, 2.0);
    commands.spawn((
        RoundEntity,
        Touchable { radius: 0.35 },
        Risk {
            label: "Session ouverte",
            action: Action::Touch(TouchEffect::LockScreen),
            success: "Session verrouillée. Le réflexe : Windows + L dès qu'on quitte son poste.",
            wrong: "",
        },
        OpenSession {
            locked: materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: LinearRgba::rgb(0.05, 0.08, 0.25),
                ..default()
            }),
        },
        Mesh3d(meshes.add(Cuboid::new(0.003, 0.34, 0.58))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(0.6, 1.4, 3.0),
            ..default()
        })),
        Transform::from_translation(screen),
    ));
    for (size, offset, color) in [
        (Vec2::new(0.16, 0.26), Vec2::new(0.05, -0.1), LinearRgba::rgb(2.6, 2.6, 2.6)),
        (Vec2::new(0.14, 0.2), Vec2::new(-0.03, 0.13), LinearRgba::rgb(0.6, 2.2, 0.8)),
        (Vec2::new(0.025, 0.58), Vec2::new(-0.155, 0.0), LinearRgba::rgb(0.1, 0.1, 0.15)),
    ] {
        commands.spawn((
            RoundEntity,
            DesktopWindow,
            Mesh3d(meshes.add(Cuboid::new(0.002, size.x, size.y))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::BLACK,
                emissive: color,
                ..default()
            })),
            Transform::from_translation(screen + Vec3::new(-0.003, offset.x, offset.y)),
        ));
    }

    // 5. Tableau blanc couvert de chiffres du projet : on le touche pour l'effacer.
    let board = Vec3::new(0.0, 1.55, -7.962);
    commands.spawn((
        RoundEntity,
        Touchable { radius: 1.4 },
        Risk {
            label: "Tableau blanc du projet",
            action: Action::Touch(TouchEffect::EraseBoard),
            success: "Tableau effacé : les chiffres du projet ne restent pas affichés.",
            wrong: "",
        },
        Transform::from_translation(board),
    ));
    let marker = materials.add(Color::srgb(0.1, 0.18, 0.55));
    let strokes = [
        (Vec2::new(1.0, 0.07), Vec2::new(-0.75, 0.4), red.clone()),
        (Vec2::new(1.2, 0.03), Vec2::new(-0.65, 0.22), marker.clone()),
        (Vec2::new(0.9, 0.03), Vec2::new(-0.8, 0.12), marker.clone()),
        (Vec2::new(1.1, 0.03), Vec2::new(-0.7, 0.02), marker.clone()),
        (Vec2::new(0.7, 0.03), Vec2::new(-0.9, -0.08), marker.clone()),
        (Vec2::new(0.45, 0.06), Vec2::new(0.85, 0.4), red),
        (Vec2::new(0.12, 0.2), Vec2::new(0.55, -0.3), marker.clone()),
        (Vec2::new(0.12, 0.35), Vec2::new(0.75, -0.225), marker.clone()),
        (Vec2::new(0.12, 0.5), Vec2::new(0.95, -0.15), marker),
    ];
    for (size, offset, material) in strokes {
        commands.spawn((
            RoundEntity,
            BoardInk,
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, 0.002))),
            MeshMaterial3d(material),
            Transform::from_translation(board + Vec3::new(offset.x, offset.y, 0.0)),
        ));
    }

    *game = Game {
        total: 5,
        started_at: now,
        message: "18 h, vous partez en dernier. Faites le tour de l'open space :\n5 risques à trouver, à prendre et à mettre dans le bon bac.".into(),
        message_until: now + 6.0,
        ..default()
    };
}

fn item_physics(collider: Collider, damping: f32) -> impl Bundle {
    (
        RigidBody::Dynamic,
        collider,
        Restitution::new(0.1),
        Friction::new(0.8),
        LinearDamping(damping),
        AngularDamping(0.6),
        SweptCcd::default(),
    )
}

// ---------- Règles ----------

fn record_success(game: &mut Game, now: f32, text: &str) {
    game.done += 1;
    game.message = text.into();
    game.message_error = false;
    game.message_until = now + 3.0;
    if game.done == game.total {
        game.finished_at = Some(now);
        game.points = points(game.errors);
    }
}

/// Un objet entre dans un bac : bon bac, il est réglé ; mauvais bac, erreur et il ressort.
pub(crate) fn sort_items(
    mut commands: Commands,
    time: Res<Time>,
    hand: Res<Hand>,
    mut game: ResMut<Game>,
    player: Single<&Transform, With<Player>>,
    receptacles: Query<(&Transform, &Receptacle), Without<Risk>>,
    mut items: Query<(Entity, &Transform, &Risk, &mut LinearVelocity, Option<&Rejected>), Without<Player>>,
) {
    let now = time.elapsed_secs();
    for (entity, transform, risk, mut velocity, rejected) in &mut items {
        let Action::Bring(expected) = risk.action else { continue };
        if hand.held == Some(entity) || rejected.is_some_and(|r| now < r.0) {
            continue;
        }
        let Some((_, receptacle)) = receptacles.iter().find(|(zone, receptacle)| {
            (transform.translation - zone.translation)
                .abs()
                .cmple(receptacle.half_extents)
                .all()
        }) else {
            continue;
        };

        if receptacle.destination == expected {
            record_success(&mut game, now, risk.success);
            if expected == Destination::Shredder {
                commands.entity(entity).despawn();
            } else {
                commands.entity(entity).remove::<(Risk, Grabbable)>();
            }
        } else {
            game.errors += 1;
            game.message = risk.wrong.into();
            game.message_error = true;
            game.message_until = now + 4.0;
            let back = (player.translation - transform.translation).with_y(0.0).normalize_or_zero();
            velocity.0 = back * 2.0 + Vec3::Y * 3.5;
            commands.entity(entity).insert(Rejected(now + 1.0));
        }
    }
}

/// Une cible touchée : on verrouille la session ou on efface le tableau.
pub(crate) fn apply_taps(
    mut commands: Commands,
    time: Res<Time>,
    mut taps: ResMut<Taps>,
    mut game: ResMut<Game>,
    risks: Query<(&Risk, Option<&OpenSession>)>,
    windows: Query<Entity, With<DesktopWindow>>,
    ink: Query<Entity, With<BoardInk>>,
) {
    let now = time.elapsed_secs();
    for entity in taps.0.drain(..) {
        let Ok((risk, session)) = risks.get(entity) else { continue };
        let Action::Touch(effect) = risk.action else { continue };
        match effect {
            TouchEffect::LockScreen => {
                windows.iter().for_each(|window| commands.entity(window).despawn());
                if let Some(session) = session {
                    commands.entity(entity).insert(MeshMaterial3d(session.locked.clone()));
                }
            }
            TouchEffect::EraseBoard => ink.iter().for_each(|stroke| commands.entity(stroke).despawn()),
        }
        record_success(&mut game, now, risk.success);
        commands.entity(entity).remove::<(Risk, Touchable)>();
    }
}

// ---------- Interface ----------

pub(crate) fn place_labels(
    camera: Single<(&Camera, &GlobalTransform)>,
    mut labels: Query<(&WorldLabel, &mut Node, &mut Visibility)>,
) {
    let (camera, camera_transform) = camera.into_inner();
    for (label, mut node, mut visibility) in &mut labels {
        match camera.world_to_viewport(camera_transform, label.0) {
            Ok(position) => {
                node.left = Val::Px(position.x - LABEL_WIDTH / 2.0);
                node.top = Val::Px(position.y);
                *visibility = Visibility::Inherited;
            }
            Err(_) => *visibility = Visibility::Hidden,
        }
    }
}

pub(crate) fn update_ui(
    time: Res<Time>,
    game: Res<Game>,
    hand: Res<Hand>,
    risks: Query<&Risk>,
    mut score: Single<&mut Text, (With<ScoreText>, Without<MessageText>, Without<HeldText>, Without<DebriefResult>)>,
    message: Single<(&mut Text, &mut TextColor), (With<MessageText>, Without<ScoreText>, Without<HeldText>, Without<DebriefResult>)>,
    mut held: Single<&mut Text, (With<HeldText>, Without<ScoreText>, Without<MessageText>, Without<DebriefResult>)>,
    mut result: Single<&mut Text, (With<DebriefResult>, Without<ScoreText>, Without<MessageText>, Without<HeldText>)>,
    mut panel: Single<&mut Visibility, With<DebriefPanel>>,
) {
    let now = time.elapsed_secs();
    let elapsed = game.finished_at.unwrap_or(now) - game.started_at;
    score.0 = format!(
        "Risques sécurisés : {} / {}\nErreurs : {}   Temps : {:.0} s",
        game.done, game.total, game.errors, elapsed
    );

    let (mut message, mut color) = message.into_inner();
    message.0 = if game.finished_at.is_none() && now < game.message_until {
        game.message.clone()
    } else {
        String::new()
    };
    color.0 = if game.message_error {
        Color::srgb(1.0, 0.45, 0.35)
    } else {
        Color::srgb(0.55, 1.0, 0.6)
    };

    held.0 = hand
        .held
        .and_then(|entity| risks.get(entity).ok())
        .map(|risk| format!("En main : {}", risk.label))
        .unwrap_or_default();

    if game.finished_at.is_some() {
        let errors = match game.errors {
            0 => "aucune erreur".to_string(),
            1 => "1 erreur".to_string(),
            n => format!("{n} erreurs"),
        };
        result.0 = format!("{elapsed:.0} s, {errors} : {} points", game.points);
        **panel = Visibility::Inherited;
    } else {
        **panel = Visibility::Hidden;
    }
}
