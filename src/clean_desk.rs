//! Mode « Bureau propre » : 18 h, on part en dernier et on sécurise cinq risques cyber et RGPD.
//! Un clic sur un objet ouvre un menu de choix, dont un piège plausible. On ne lance plus qu'à la
//! broyeuse ; ce qu'on garde se range dans la grande armoire, dont les portes s'ouvrent à l'approche.
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    Bodies, ContextMenu, Game, Grabbable, Hand, MenuChoices, MessageText, NATIVE, Obstacle, Player,
    RoundEntity, ScoreText, Taps, Touchable, UsableWhileHolding, grab, uses_mouse,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Destination {
    Shredder,
    Cabinet,
}

/// Zone de dépôt : une boîte de détection centrée sur le Transform de l'entité.
#[derive(Component)]
pub(crate) struct Receptacle {
    pub(crate) destination: Destination,
    pub(crate) half_extents: Vec3,
}

/// Ce que fait un choix du menu.
#[derive(Clone, Copy)]
pub(crate) enum Outcome {
    Take,
    Right(&'static str, Effect),
    Wrong(&'static str),
}

#[derive(Clone, Copy)]
pub(crate) enum Effect {
    LockScreen,
    EraseBoard,
    SendToIt,
}

#[derive(Component)]
pub(crate) struct Risk {
    pub(crate) label: &'static str,
    /// Objets à prendre : où ils doivent finir (broyeuse ou armoire).
    pub(crate) destination: Option<Destination>,
    /// Bonne destination atteinte.
    success: &'static str,
    /// Mauvaise destination : pourquoi.
    wrong: &'static str,
    /// Dans l'ordre des lignes du menu.
    pub(crate) outcomes: Vec<Outcome>,
}

/// Renvoyé par la broyeuse : on ne le reteste pas pendant qu'il en ressort.
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

/// La grande armoire : on clique dessus, un objet en main, pour l'y ranger.
#[derive(Component)]
pub(crate) struct Armoire {
    stored: usize,
    /// 0 fermée, 1 grande ouverte.
    openness: f32,
}

/// Porte de l'armoire, tournant sur sa charnière (son Transform) jusqu'à cet angle.
#[derive(Component)]
pub(crate) struct Door {
    open_angle: f32,
}

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
const ARMOIRE: Vec3 = Vec3::new(-3.2, 0.0, 7.72);
const ARMOIRE_SIZE: Vec3 = Vec3::new(1.4, 2.0, 0.5);
const SHELVES: [f32; 3] = [0.55, 1.05, 1.5];
/// Les portes s'ouvrent quand on est à moins de cette distance (au sol) de l'armoire.
const DOORS_RADIUS: f32 = 2.6;

const RULES: &str = "\u{2022} Mot de passe écrit : à la broyeuse, puis on le change.\n\
\u{2022} Session : on la verrouille (Windows + L) ; éteindre l'écran ne suffit pas.\n\
\u{2022} Document confidentiel : rangé sous clé, jamais laissé à l'imprimante.\n\
\u{2022} Clé USB inconnue : jamais branchée, envoyée au service informatique.\n\
\u{2022} Tableau blanc : effacé, pas photographié.";

pub(crate) fn points(errors: u32) -> u32 {
    2000u32.saturating_sub(300 * errors).max(500)
}

/// Libellés du menu et effets, dans le même ordre.
fn choices(
    title: &'static str,
    options: &[(&'static str, Outcome)],
) -> (MenuChoices, Vec<Outcome>) {
    (
        MenuChoices {
            title,
            labels: options.iter().map(|(label, _)| *label).collect(),
        },
        options.iter().map(|(_, outcome)| *outcome).collect(),
    )
}

// ---------- Décor fixe : broyeuse, armoire, imprimante, étiquettes ----------

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

/// Bac ouvert : un fond et quatre bords, posé avec son fond à la hauteur de `center`.
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

/// Armoire contre le mur du fond, ouverte vers la pièce : caisson, étagères, deux portes.
fn spawn_armoire(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let body = materials.add(StandardMaterial {
        base_color: Color::srgb(0.7, 0.72, 0.75),
        metallic: 0.4,
        perceptual_roughness: 0.45,
        ..default()
    });
    let inside = materials.add(Color::srgb(0.45, 0.47, 0.5));
    let handle = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.7, 0.2),
        metallic: 0.9,
        perceptual_roughness: 0.3,
        ..default()
    });

    let Vec3 { x: w, y: h, z: d } = ARMOIRE_SIZE;
    let t = 0.03;
    let at = |x: f32, y: f32, z: f32| ARMOIRE + Vec3::new(x, y, z);
    spawn_static_box(commands, meshes, &inside, Vec3::new(w, h, t), at(0.0, h / 2.0, d / 2.0 - t / 2.0));
    spawn_static_box(commands, meshes, &body, Vec3::new(t, h, d), at(-w / 2.0 + t / 2.0, h / 2.0, 0.0));
    spawn_static_box(commands, meshes, &body, Vec3::new(t, h, d), at(w / 2.0 - t / 2.0, h / 2.0, 0.0));
    spawn_static_box(commands, meshes, &body, Vec3::new(w, t, d), at(0.0, h - t / 2.0, 0.0));
    spawn_static_box(commands, meshes, &body, Vec3::new(w, 0.06, d), at(0.0, 0.03, 0.0));
    for y in SHELVES {
        spawn_static_box(commands, meshes, &inside, Vec3::new(w - 2.0 * t, 0.025, d - 0.04), at(0.0, y, 0.02));
    }

    // Charnières aux deux coins avant ; chaque porte s'ouvre vers la pièce (z décroissant).
    let door_mesh = meshes.add(Cuboid::new(w / 2.0 - 0.005, h - 0.04, 0.025));
    let handle_mesh = meshes.add(Cuboid::new(0.02, 0.22, 0.03));
    for side in [-1.0f32, 1.0] {
        commands
            .spawn((
                Door {
                    open_angle: -side * 1.75,
                },
                Transform::from_translation(at(side * w / 2.0, h / 2.0, -d / 2.0 - 0.015)),
                Visibility::default(),
            ))
            .with_children(|door| {
                door.spawn((
                    Mesh3d(door_mesh.clone()),
                    MeshMaterial3d(body.clone()),
                    Transform::from_xyz(-side * w / 4.0, 0.0, 0.0),
                ));
                door.spawn((
                    Mesh3d(handle_mesh.clone()),
                    MeshMaterial3d(handle.clone()),
                    Transform::from_xyz(-side * (w / 2.0 - 0.07), 0.05, -0.025),
                ));
            });
    }

    // Cible cliquable : le devant des étagères. Un objet en main, un clic l'y range.
    commands.spawn((
        Armoire {
            stored: 0,
            openness: 0.0,
        },
        Touchable { radius: 0.8 },
        UsableWhileHolding,
        Transform::from_translation(at(0.0, 1.1, -0.1)),
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

    // Broyeuse : bloc sombre, fente sur le dessus, bac peu profond pour retenir ce qu'on y lance.
    let shredder = Vec3::new(-1.2, 0.0, 3.8);
    spawn_static_box(&mut commands, &mut meshes, &dark, Vec3::new(0.45, 0.6, 0.32), shredder + Vec3::Y * 0.3);
    spawn_tray(&mut commands, &mut meshes, &dark, shredder + Vec3::Y * 0.6, Vec2::new(0.45, 0.32), 0.06, Destination::Shredder);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.32, 0.005, 0.03))),
        MeshMaterial3d(materials.add(Color::BLACK)),
        Transform::from_translation(shredder + Vec3::Y * 0.623),
    ));

    spawn_armoire(&mut commands, &mut meshes, &mut materials);

    // Imprimante contre le mur du fond, derrière le joueur.
    let printer = Vec3::new(3.6, 0.0, 7.55);
    spawn_static_box(&mut commands, &mut meshes, &steel, Vec3::new(0.9, 0.72, 0.5), printer + Vec3::Y * 0.36);
    spawn_static_box(&mut commands, &mut meshes, &white, Vec3::new(0.55, 0.3, 0.42), printer + Vec3::Y * 0.87);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.4, 0.004, 0.12))),
        MeshMaterial3d(dark.clone()),
        Transform::from_translation(printer + Vec3::new(0.0, 1.022, 0.12)),
    ));

    // On contourne les meubles : le joueur est repoussé à cette distance de leur centre. Devant
    // l'armoire, assez loin pour la voir en entier, portes ouvertes.
    for (center, radius) in [(shredder, 0.45), (ARMOIRE, 1.1), (printer, 0.75)] {
        commands.spawn((Transform::from_translation(center), Obstacle { radius }));
    }

    for (text, anchor) in [
        ("Broyeuse", shredder + Vec3::Y * 1.0),
        ("Armoire fermée à clé", ARMOIRE + Vec3::Y * 2.25),
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
                Text::new(if NATIVE {
                    "Clic ou R pour rejouer"
                } else {
                    "Touchez ou cliquez pour continuer"
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
    let (menu, outcomes) = choices(
        "Post-it avec un mot de passe",
        &[
            ("Prendre", Outcome::Take),
            (
                "Le cacher sous le clavier",
                Outcome::Wrong("Sous le clavier, c'est le premier endroit où l'on cherche un mot de passe."),
            ),
        ],
    );
    commands.spawn((
        RoundEntity,
        Grabbable,
        menu,
        Risk {
            label: "Post-it avec un mot de passe",
            destination: Some(Destination::Shredder),
            success: "Mot de passe détruit. Pensez aussi à le changer.",
            wrong: "Un mot de passe écrit ne se garde pas, même sous clé : à la broyeuse, puis on le change.",
            outcomes,
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
    let (menu, outcomes) = choices(
        "Contrat client signé, confidentiel",
        &[
            (
                "Le retourner face cachée",
                Outcome::Wrong("Face cachée, il reste lisible par le premier venu : un contrat signé se range sous clé."),
            ),
            ("Prendre", Outcome::Take),
        ],
    );
    commands
        .spawn((
            RoundEntity,
            Grabbable,
            menu,
            Risk {
                label: "Contrat client signé, confidentiel",
                destination: Some(Destination::Cabinet),
                success: "Contrat rangé sous clé.",
                wrong: "Un contrat signé est un original : il se range sous clé, il ne se détruit pas.",
                outcomes,
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

    // 3. Clé USB inconnue posée à côté du PC resté allumé (bureau de droite) : elle ne se prend
    // pas, elle part au service informatique. À droite de l'écran, pour ne pas le masquer au clic.
    let (menu, outcomes) = choices(
        "Clé USB inconnue près du PC",
        &[
            (
                "La brancher pour trouver son propriétaire",
                Outcome::Wrong("Une clé inconnue peut contenir un logiciel malveillant : on ne la branche jamais."),
            ),
            (
                "L'envoyer au service informatique",
                Outcome::Right("Clé envoyée au service informatique, sans la brancher.", Effect::SendToIt),
            ),
        ],
    );
    commands
        .spawn((
            RoundEntity,
            Touchable { radius: 0.06 },
            menu,
            Risk {
                label: "Clé USB inconnue près du PC",
                destination: None,
                success: "",
                wrong: "",
                outcomes,
            },
            Mesh3d(meshes.add(Cuboid::new(0.075, 0.016, 0.026))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.35, 0.08),
                emissive: LinearRgba::rgb(0.15, 0.04, 0.0),
                ..default()
            })),
            Transform::from_xyz(5.6, 0.768, 2.47).with_rotation(Quat::from_rotation_y(0.4)),
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
    let (menu, outcomes) = choices(
        "Session ouverte",
        &[
            (
                "Éteindre l'écran",
                Outcome::Wrong("Éteindre l'écran ne ferme pas la session : n'importe qui peut le rallumer et s'en servir."),
            ),
            (
                "Verrouiller la session",
                Outcome::Right(
                    "Session verrouillée. Le réflexe : Windows + L dès qu'on quitte son poste.",
                    Effect::LockScreen,
                ),
            ),
        ],
    );
    commands.spawn((
        RoundEntity,
        Touchable { radius: 0.35 },
        menu,
        Risk {
            label: "Session ouverte",
            destination: None,
            success: "",
            wrong: "",
            outcomes,
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

    // 5. Tableau blanc couvert de chiffres du projet.
    let board = Vec3::new(0.0, 1.55, -7.962);
    let (menu, outcomes) = choices(
        "Tableau blanc du projet",
        &[
            (
                "Effacer",
                Outcome::Right("Tableau effacé : les chiffres du projet ne restent pas affichés.", Effect::EraseBoard),
            ),
            (
                "Le prendre en photo pour plus tard",
                Outcome::Wrong("Des chiffres confidentiels photographiés sur un téléphone, c'est une fuite de données en puissance."),
            ),
        ],
    );
    commands.spawn((
        RoundEntity,
        Touchable { radius: 1.4 },
        menu,
        Risk {
            label: "Tableau blanc du projet",
            destination: None,
            success: "",
            wrong: "",
            outcomes,
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

    let act = if uses_mouse() { "Cliquez" } else { "Touchez" };
    *game = Game {
        total: 5,
        started_at: now,
        message: format!(
            "18 h, vous partez en dernier. Faites le tour de l'open space :\n5 risques à trouver. {act} sur un objet pour choisir quoi en faire."
        ),
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

fn record_error(game: &mut Game, now: f32, text: &str) {
    game.errors += 1;
    game.message = text.into();
    game.message_error = true;
    game.message_until = now + 4.0;
}

/// Un choix du menu : prendre l'objet, ou le bon geste (réglé), ou le piège (erreur).
pub(crate) fn resolve_menu(
    mut commands: Commands,
    time: Res<Time>,
    mut menu: ResMut<ContextMenu>,
    mut game: ResMut<Game>,
    mut hand: ResMut<Hand>,
    mut bodies: Bodies,
    risks: Query<(&Risk, Option<&OpenSession>)>,
    windows: Query<Entity, With<DesktopWindow>>,
    ink: Query<Entity, With<BoardInk>>,
) {
    let Some((entity, index)) = menu.chosen.take() else { return };
    let Ok((risk, session)) = risks.get(entity) else { return };
    let now = time.elapsed_secs();
    match risk.outcomes.get(index) {
        // Pas armé : le clic qui a choisi « Prendre » ne doit pas préparer un lancer.
        Some(Outcome::Take) => grab(&mut hand, &mut bodies, entity, false),
        Some(Outcome::Wrong(text)) => record_error(&mut game, now, text),
        Some(Outcome::Right(text, effect)) => {
            match effect {
                Effect::LockScreen => {
                    windows.iter().for_each(|window| commands.entity(window).despawn());
                    if let Some(session) = session {
                        commands.entity(entity).insert(MeshMaterial3d(session.locked.clone()));
                    }
                }
                Effect::EraseBoard => ink.iter().for_each(|stroke| commands.entity(stroke).despawn()),
                Effect::SendToIt => {}
            }
            record_success(&mut game, now, text);
            match effect {
                Effect::SendToIt => commands.entity(entity).despawn(),
                _ => {
                    commands.entity(entity).remove::<(Risk, MenuChoices, Touchable)>();
                }
            }
        }
        None => {}
    }
}

/// Un objet lancé entre dans la broyeuse : à détruire, c'est réglé ; sinon erreur et il ressort.
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
        let Some(expected) = risk.destination else { continue };
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
            commands.entity(entity).despawn();
        } else {
            record_error(&mut game, now, risk.wrong);
            let back = (player.translation - transform.translation).with_y(0.0).normalize_or_zero();
            velocity.0 = back * 2.0 + Vec3::Y * 3.5;
            commands.entity(entity).insert(Rejected(now + 1.0));
        }
    }
}

/// Clic sur l'armoire, un objet en main : on le pose sur l'étagère du milieu. Seul ce qui se garde
/// sous clé y a sa place ; le reste est une erreur, et l'objet reste en main.
pub(crate) fn use_armoire(
    mut commands: Commands,
    time: Res<Time>,
    mut taps: ResMut<Taps>,
    mut hand: ResMut<Hand>,
    mut game: ResMut<Game>,
    mut armoires: Query<(&Transform, &mut Armoire)>,
    risks: Query<&Risk>,
    mut bodies: Query<(&mut Position, &mut LinearVelocity, &mut AngularVelocity, &mut GravityScale)>,
) {
    let now = time.elapsed_secs();
    for entity in taps.0.drain(..) {
        let Ok((transform, mut armoire)) = armoires.get_mut(entity) else { continue };
        let Some(held) = hand.held else {
            game.message = "L'armoire sert à ranger : prenez d'abord un objet.".into();
            game.message_error = false;
            game.message_until = now + 2.0;
            continue;
        };
        let Ok(risk) = risks.get(held) else { continue };
        if risk.destination != Some(Destination::Cabinet) {
            record_error(&mut game, now, risk.wrong);
            continue;
        }
        if let Ok((mut position, mut velocity, mut spin, mut gravity)) = bodies.get_mut(held) {
            let slot = (armoire.stored % 3) as f32 - 1.0;
            position.0 = Vec3::new(transform.translation.x + slot * 0.4, SHELVES[1] + 0.05, ARMOIRE.z);
            velocity.0 = Vec3::ZERO;
            spin.0 = Vec3::ZERO;
            gravity.0 = 1.0;
        }
        armoire.stored += 1;
        *hand = Hand::default();
        record_success(&mut game, now, risk.success);
        commands.entity(held).remove::<(Risk, Grabbable, MenuChoices)>();
    }
}

/// Les portes s'ouvrent quand on s'approche de l'armoire, et se referment quand on s'en va.
pub(crate) fn animate_doors(
    time: Res<Time>,
    player: Single<&Transform, With<Player>>,
    mut armoire: Single<&mut Armoire>,
    mut doors: Query<(&mut Transform, &Door), Without<Player>>,
) {
    let near = Vec2::new(player.translation.x - ARMOIRE.x, player.translation.z - ARMOIRE.z).length() < DOORS_RADIUS;
    let goal = if near { 1.0 } else { 0.0 };
    let step = time.delta_secs() * 1.8;
    armoire.openness += (goal - armoire.openness).clamp(-step, step);
    // Démarrage et arrivée en douceur.
    let eased = armoire.openness * armoire.openness * (3.0 - 2.0 * armoire.openness);
    for (mut transform, door) in &mut doors {
        transform.rotation = Quat::from_rotation_y(door.open_angle * eased);
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
