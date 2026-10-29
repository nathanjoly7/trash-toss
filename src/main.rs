#[cfg(feature = "autoplay")]
mod autoplay;

use std::f32::consts::{FRAC_PI_2, TAU};

use avian3d::prelude::*;
use bevy::{
    input::{mouse::AccumulatedMouseMotion, touch::Touches},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

/// Natif : vue FPS, souris capturée. Web (souvent dans une iframe GeoGaming, souvent au doigt) :
/// pas de capture, on glisse pour regarder et on touche un déchet pour le ramasser.
const USE_POINTER_LOCK: bool = !cfg!(target_arch = "wasm32");
const DRAG_THRESHOLD: f32 = 12.0;
/// Un déchet de 7 cm est trop petit pour un doigt : on prend le plus proche à l'écran.
const TOUCH_TOLERANCE: f32 = 56.0;
const CROSSHAIR_TOLERANCE: f32 = 40.0;
const DRAG_SENSITIVITY: f32 = 0.006;
/// Champ horizontal minimal : en écran vertical (téléphone), on élargit le champ vertical.
const MIN_HALF_HFOV: f32 = 0.49;

const ROOM_HALF_X: f32 = 6.0;
const ROOM_HALF_Z: f32 = 8.0;
const ROOM_HEIGHT: f32 = 3.0;
const EYE_HEIGHT: f32 = 1.65;
/// Ligne jaune au sol : on lance depuis l'arrière, la poubelle est à 2,5 m au plus près.
const THROW_LINE_Z: f32 = 3.0;
const WALK_SPEED: f32 = 3.5;
const MOUSE_SENSITIVITY: f32 = 0.0022;

const BIN_POS: Vec3 = Vec3::new(0.0, 0.0, 0.5);
const BIN_INNER: f32 = 0.32;
const BIN_OUTER: f32 = 0.35;
const BIN_HEIGHT: f32 = 0.65;

/// Le déchet visé vole jusqu'à la main : au téléphone, on ne se déplace pas.
const REACH: f32 = 8.0;
const CARRY_MAX_SPEED: f32 = 12.0;
const MIN_THROW_SPEED: f32 = 3.0;
const MAX_THROW_SPEED: f32 = 12.0;
const FULL_CHARGE_SECS: f32 = 1.0;

const CROSSHAIR_IDLE: Color = Color::srgba(1.0, 1.0, 1.0, 0.6);
const CROSSHAIR_TARGET: Color = Color::srgb(1.0, 0.8, 0.1);

#[derive(Component)]
struct Player {
    yaw: f32,
    pitch: f32,
}

#[derive(Component)]
struct Trash;

#[derive(Component)]
struct InBin;

#[derive(Component)]
struct Crosshair;

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct MessageText;

#[derive(Component)]
struct PowerBar;

#[derive(Component)]
struct PowerFill;

#[derive(Resource, Default)]
struct Hand {
    held: Option<Entity>,
    /// Le clic qui ramasse ne doit pas lancer : on attend qu'il soit relâché.
    armed: bool,
    charge_start: Option<f32>,
    /// Arrivé une fois devant la caméra : s'il s'en éloigne ensuite, il est coincé.
    arrived: bool,
}

#[derive(Resource, Default)]
struct Game {
    total: usize,
    in_bin: usize,
    started_at: f32,
    finished_at: Option<f32>,
    message_until: f32,
    notified: bool,
    restart_requested: bool,
}

/// Geste en cours (doigt, ou souris non capturée sur le web).
#[derive(Resource, Default)]
struct Gesture {
    start: Option<Vec2>,
    last: Vec2,
    dragging: bool,
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::prelude::*;

    // Contrat des activités Widget GeoGaming : le pont de l'app relaie ce message et valide l'étape.
    // Le bouton « Passer » du cadre envoie le même message avec 0 point.
    #[wasm_bindgen(inline_js = "export function notify_completed(seconds) { \
        window.parent.postMessage({ completed: true, points: 2000, activityKey: 'trash-toss', \
        seconds: Math.round(seconds) }, '*'); } \
        export function canvas_css_width() { return document.getElementById('bevy')?.clientWidth ?? 0; } \
        export function canvas_css_height() { return document.getElementById('bevy')?.clientHeight ?? 0; }")]
    extern "C" {
        pub fn notify_completed(seconds: f32);
        pub fn canvas_css_width() -> f64;
        pub fn canvas_css_height() -> f64;
    }
}

/// Passe d'une position du doigt (pixels CSS) aux pixels logiques de Bevy. Les deux sont égaux
/// d'habitude, mais quand le navigateur mesure mal le canvas (vu en émulation mobile, densité 2),
/// le doigt viserait à côté.
fn pointer_scale(window: &Window) -> Vec2 {
    #[cfg(target_arch = "wasm32")]
    {
        let css = Vec2::new(web::canvas_css_width() as f32, web::canvas_css_height() as f32);
        if css.min_element() > 0.0 {
            return window.size() / css;
        }
    }
    let _ = window;
    Vec2::ONE
}

#[derive(Clone, Copy)]
enum Kind {
    Paper,
    Can,
    Bottle,
    Cup,
}

fn main() {
    let mut app = App::new();
    app.add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Trash Toss".into(),
                    canvas: Some("#bevy".into()),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            }),
            PhysicsPlugins::default().set(PhysicsInterpolationPlugin::interpolate_all()),
        ))
        .insert_resource(GlobalAmbientLight {
            brightness: 300.0,
            ..default()
        })
        .init_resource::<Hand>()
        .init_resource::<Game>()
        .init_resource::<Gesture>()
        .add_systems(
            Startup,
            (
                use_embedded_font,
                setup_office,
                setup_bin,
                setup_player,
                setup_ui,
                setup_trash,
                lock_cursor,
            ),
        )
        .add_systems(
            Update,
            (
                fit_fov,
                look_and_walk,
                handle_pointer,
                carry_held,
                detect_in_bin,
                notify_geogaming,
                update_ui,
                toggle_cursor,
                restart,
            )
                .chain(),
        );
    #[cfg(feature = "autoplay")]
    app.add_systems(Update, autoplay::autoplay);
    app.run();
}

// ---------- Décor ----------

fn spawn_block(
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

fn setup_office(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let carpet = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.34, 0.40),
        perceptual_roughness: 1.0,
        ..default()
    });
    let wall = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.85, 0.81),
        perceptual_roughness: 0.9,
        ..default()
    });
    let ceiling = materials.add(Color::srgb(0.93, 0.93, 0.93));
    let glass = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(2.6, 3.2, 4.2),
        ..default()
    });
    let lamp = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::rgb(6.0, 6.0, 5.6),
        ..default()
    });
    let whiteboard = materials.add(StandardMaterial {
        base_color: Color::srgb(0.97, 0.97, 0.97),
        perceptual_roughness: 0.15,
        ..default()
    });

    let (hx, hz, h, t) = (ROOM_HALF_X, ROOM_HALF_Z, ROOM_HEIGHT, 0.2);
    spawn_block(&mut commands, &mut meshes, &carpet, Vec3::new(2.0 * hx, t, 2.0 * hz), Vec3::new(0.0, -t / 2.0, 0.0));
    spawn_block(&mut commands, &mut meshes, &ceiling, Vec3::new(2.0 * hx, t, 2.0 * hz), Vec3::new(0.0, h + t / 2.0, 0.0));
    spawn_block(&mut commands, &mut meshes, &wall, Vec3::new(2.0 * hx, h, t), Vec3::new(0.0, h / 2.0, -hz - t / 2.0));
    spawn_block(&mut commands, &mut meshes, &wall, Vec3::new(2.0 * hx, h, t), Vec3::new(0.0, h / 2.0, hz + t / 2.0));
    spawn_block(&mut commands, &mut meshes, &wall, Vec3::new(t, h, 2.0 * hz), Vec3::new(-hx - t / 2.0, h / 2.0, 0.0));
    spawn_block(&mut commands, &mut meshes, &wall, Vec3::new(t, h, 2.0 * hz), Vec3::new(hx + t / 2.0, h / 2.0, 0.0));

    // Baies vitrées lumineuses sur le mur de gauche, avec leurs montants.
    let frame = materials.add(Color::srgb(0.25, 0.26, 0.28));
    for z in [-5.5, -2.0, 1.5, 5.0] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.02, 1.3, 2.6))),
            MeshMaterial3d(glass.clone()),
            Transform::from_xyz(-hx + 0.01, 1.6, z),
        ));
        for size in [Vec3::new(0.05, 1.3, 0.05), Vec3::new(0.05, 0.05, 2.6)] {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(size))),
                MeshMaterial3d(frame.clone()),
                Transform::from_xyz(-hx + 0.02, 1.6, z),
            ));
        }
    }

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0 * hx, 0.004, 0.06))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.1),
            emissive: LinearRgba::rgb(0.3, 0.24, 0.03),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.002, THROW_LINE_Z),
    ));

    // Tableau blanc au fond.
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(3.0, 1.2, 0.03))),
        MeshMaterial3d(whiteboard),
        Transform::from_xyz(0.0, 1.55, -hz + 0.02),
    ));

    // Dalles lumineuses et plafonniers.
    for (i, (x, z)) in [(-2.5, -4.0), (2.5, -4.0), (-2.5, 2.0), (2.5, 2.0)].into_iter().enumerate() {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.2, 0.03, 0.6))),
            MeshMaterial3d(lamp.clone()),
            Transform::from_xyz(x, h - 0.015, z),
        ));
        commands.spawn((
            PointLight {
                intensity: 250_000.0,
                range: 14.0,
                // Quatre ombres omnidirectionnelles coûtent trop cher sur un téléphone.
                shadow_maps_enabled: USE_POINTER_LOCK || i == 3,
                ..default()
            },
            Transform::from_xyz(x, h - 0.35, z),
        ));
    }

    // Bureaux contre les murs, écrans tournés vers le centre.
    for z in [-5.0, -2.0, 2.0] {
        spawn_desk(&mut commands, &mut meshes, &mut materials, Vec3::new(-hx + 0.5, 0.0, z), FRAC_PI_2);
        spawn_desk(&mut commands, &mut meshes, &mut materials, Vec3::new(hx - 0.5, 0.0, z), -FRAC_PI_2);
    }

    // Plantes dans les coins du fond.
    let pot = materials.add(Color::srgb(0.35, 0.25, 0.2));
    let leaves = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.45, 0.2),
        perceptual_roughness: 0.8,
        ..default()
    });
    for x in [-hx + 0.5, hx - 0.5] {
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.25, 0.45))),
            MeshMaterial3d(pot.clone()),
            Transform::from_xyz(x, 0.225, -hz + 0.5),
            RigidBody::Static,
            Collider::cylinder(0.25, 0.45),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.45).mesh().ico(2).unwrap())),
            MeshMaterial3d(leaves.clone()),
            Transform::from_xyz(x, 0.95, -hz + 0.5).with_scale(Vec3::new(1.0, 1.3, 1.0)),
        ));
    }
}

fn spawn_desk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    position: Vec3,
    angle: f32,
) {
    let top = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.91, 0.88),
        perceptual_roughness: 0.5,
        ..default()
    });
    let metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.23, 0.25),
        metallic: 0.6,
        perceptual_roughness: 0.4,
        ..default()
    });
    let screen = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.5, 1.0, 2.0),
        ..default()
    });
    let fabric = materials.add(Color::srgb(0.15, 0.2, 0.32));

    // Repère local : plateau le long de x, écran au fond (z négatif) tourné vers +z.
    let parts = [
        (Vec3::new(1.6, 0.04, 0.8), Vec3::new(0.0, 0.74, 0.0), top.clone()),
        (Vec3::new(0.04, 0.72, 0.75), Vec3::new(-0.76, 0.36, 0.0), metal.clone()),
        (Vec3::new(0.04, 0.72, 0.75), Vec3::new(0.76, 0.36, 0.0), metal.clone()),
        (Vec3::new(0.06, 0.25, 0.06), Vec3::new(0.0, 0.885, -0.25), metal.clone()),
        (Vec3::new(0.62, 0.38, 0.03), Vec3::new(0.0, 1.13, -0.25), metal.clone()),
        (Vec3::new(0.58, 0.34, 0.005), Vec3::new(0.0, 1.13, -0.233), screen),
        (Vec3::new(0.48, 0.06, 0.46), Vec3::new(0.0, 0.46, 0.6), fabric.clone()),
        (Vec3::new(0.48, 0.5, 0.06), Vec3::new(0.0, 0.76, 0.84), fabric),
        (Vec3::new(0.05, 0.43, 0.05), Vec3::new(0.0, 0.215, 0.6), metal),
    ];

    commands
        .spawn((
            Transform::from_translation(position).with_rotation(Quat::from_rotation_y(angle)),
            Visibility::default(),
            RigidBody::Static,
        ))
        .with_children(|desk| {
            for (size, offset, material) in parts {
                desk.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(size))),
                    MeshMaterial3d(material),
                    Transform::from_translation(offset),
                    Collider::cuboid(size.x, size.y, size.z),
                ));
            }
        });
}

fn setup_bin(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let plastic = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.5, 0.28),
        perceptual_roughness: 0.45,
        ..default()
    });

    // Visuel : un tube creux (anneau extrudé, couché sur z puis redressé).
    commands.spawn((
        Mesh3d(meshes.add(Extrusion::new(Annulus::new(BIN_INNER, BIN_OUTER), BIN_HEIGHT))),
        MeshMaterial3d(plastic.clone()),
        Transform::from_translation(BIN_POS + Vec3::Y * BIN_HEIGHT / 2.0)
            .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(BIN_OUTER, 0.03))),
        MeshMaterial3d(plastic),
        Transform::from_translation(BIN_POS + Vec3::Y * 0.015),
        RigidBody::Static,
        Collider::cylinder(BIN_OUTER, 0.03),
    ));

    // Paroi physique : un anneau de planches invisibles, plus robuste qu'un trimesh fin.
    let segments = 20;
    let radius = (BIN_INNER + BIN_OUTER) / 2.0;
    let width = TAU * BIN_OUTER / segments as f32 * 1.1;
    for i in 0..segments {
        let angle = i as f32 / segments as f32 * TAU;
        commands.spawn((
            Transform::from_translation(
                BIN_POS + Vec3::new(angle.cos() * radius, BIN_HEIGHT / 2.0, angle.sin() * radius),
            )
            .with_rotation(Quat::from_rotation_y(-angle)),
            RigidBody::Static,
            Collider::cuboid(0.04, BIN_HEIGHT, width),
        ));
    }
}

// ---------- Déchets ----------

fn setup_trash(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut game: ResMut<Game>,
    time: Res<Time>,
) {
    spawn_trash(&mut commands, &mut meshes, &mut materials, &mut game, time.elapsed_secs());
}

fn spawn_trash(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    game: &mut Game,
    now: f32,
) {
    let items = [
        (Kind::Paper, Vec3::new(-1.5, 0.1, 3.6)),
        (Kind::Can, Vec3::new(1.2, 0.1, 3.9)),
        (Kind::Bottle, Vec3::new(-0.4, 0.1, 4.6)),
        (Kind::Paper, Vec3::new(2.4, 0.1, 4.4)),
        (Kind::Cup, Vec3::new(-2.8, 0.1, 5.0)),
        (Kind::Can, Vec3::new(0.8, 0.1, 5.6)),
        (Kind::Paper, Vec3::new(-1.8, 0.1, 7.0)),
        (Kind::Cup, Vec3::new(3.0, 0.1, 6.8)),
        (Kind::Paper, Vec3::new(-5.35, 0.85, 2.5)),
        (Kind::Bottle, Vec3::new(5.35, 0.85, 2.4)),
    ];

    for (i, (kind, position)) in items.into_iter().enumerate() {
        let (mesh, material, collider, restitution, damping) = match kind {
            Kind::Paper => (
                Sphere::new(0.07)
                    .mesh()
                    .ico(1)
                    .unwrap()
                    .with_duplicated_vertices()
                    .with_computed_flat_normals(),
                StandardMaterial {
                    base_color: Color::srgb(0.95, 0.95, 0.92),
                    perceptual_roughness: 1.0,
                    ..default()
                },
                Collider::sphere(0.07),
                0.25,
                0.3,
            ),
            Kind::Can => (
                Cylinder::new(0.033, 0.12).into(),
                StandardMaterial {
                    base_color: Color::srgb(0.8, 0.08, 0.1),
                    metallic: 0.8,
                    perceptual_roughness: 0.3,
                    ..default()
                },
                Collider::cylinder(0.033, 0.12),
                0.35,
                0.05,
            ),
            Kind::Bottle => (
                Capsule3d::new(0.035, 0.16).into(),
                StandardMaterial {
                    base_color: Color::srgba(0.55, 0.8, 0.95, 0.55),
                    alpha_mode: AlphaMode::Blend,
                    perceptual_roughness: 0.1,
                    ..default()
                },
                Collider::capsule(0.035, 0.16),
                0.4,
                0.05,
            ),
            Kind::Cup => (
                ConicalFrustum {
                    radius_top: 0.045,
                    radius_bottom: 0.032,
                    height: 0.1,
                }
                .into(),
                StandardMaterial {
                    base_color: Color::srgb(0.9, 0.84, 0.72),
                    perceptual_roughness: 0.8,
                    ..default()
                },
                Collider::cylinder(0.04, 0.1),
                0.15,
                0.15,
            ),
        };

        let k = i as f32;
        commands.spawn((
            Trash,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(material)),
            Transform::from_translation(position)
                .with_rotation(Quat::from_euler(EulerRot::XYZ, k * 1.3, k * 0.7, k * 2.1)),
            RigidBody::Dynamic,
            collider,
            Restitution::new(restitution),
            Friction::new(0.6),
            LinearDamping(damping),
            AngularDamping(0.4),
            GravityScale(1.0),
            SweptCcd::default(),
        ));
    }

    *game = Game {
        total: items.len(),
        started_at: now,
        ..default()
    };
}

// ---------- Joueur ----------

fn setup_player(mut commands: Commands) {
    commands.spawn((
        Player {
            yaw: 0.0,
            pitch: -0.3,
        },
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: 65f32.to_radians(),
            ..default()
        }),
        // Écrans de téléphone très denses : l'anticrénelage y coûte plus qu'il ne rapporte.
        if USE_POINTER_LOCK { Msaa::Sample4 } else { Msaa::Off },
        Transform::from_xyz(0.0, EYE_HEIGHT, 6.5),
    ));
}

/// La police intégrée à Bevy n'a pas les accents : on la remplace par Montserrat (OFL, police de l'app).
fn use_embedded_font(mut fonts: ResMut<Assets<Font>>) {
    let bytes = include_bytes!("../assets/fonts/Montserrat-SemiBold.ttf");
    fonts.insert(AssetId::default(), Font::from_bytes(bytes.to_vec())).ok();
}

fn fit_fov(window: Single<&Window>, mut projection: Single<&mut Projection, With<Player>>) {
    let aspect = window.width() / window.height().max(1.0);
    let fov = (2.0 * (MIN_HALF_HFOV.tan() / aspect).atan())
        .clamp(65f32.to_radians(), 95f32.to_radians());
    if let Projection::Perspective(perspective) = &mut **projection
        && (perspective.fov - fov).abs() > 0.001
    {
        perspective.fov = fov;
    }
}

fn pointer_locked(cursor: &CursorOptions) -> bool {
    USE_POINTER_LOCK && cursor.grab_mode != CursorGrabMode::None
}

fn lock_cursor(mut cursor: Single<&mut CursorOptions>) {
    if USE_POINTER_LOCK {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

/// Échap libère la souris, un clic dans la fenêtre la reprend.
fn toggle_cursor(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: Single<&mut CursorOptions>,
) {
    if !USE_POINTER_LOCK {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if mouse.just_pressed(MouseButton::Left) && cursor.grab_mode == CursorGrabMode::None {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

/// ZQSD en AZERTY (KeyCode suit la position physique) ou flèches.
fn look_and_walk(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions>,
    player: Single<(&mut Player, &mut Transform)>,
) {
    let (mut player, mut transform) = player.into_inner();
    if pointer_locked(&cursor) {
        player.yaw -= mouse_motion.delta.x * MOUSE_SENSITIVITY;
        player.pitch = (player.pitch - mouse_motion.delta.y * MOUSE_SENSITIVITY).clamp(-1.5, 1.5);
    }
    transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);

    let forward = Vec3::new(-player.yaw.sin(), 0.0, -player.yaw.cos());
    let right = Vec3::new(player.yaw.cos(), 0.0, -player.yaw.sin());
    let mut direction = Vec3::ZERO;
    if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        direction += forward;
    }
    if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        direction -= forward;
    }
    if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        direction += right;
    }
    if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        direction -= right;
    }
    transform.translation += direction.normalize_or_zero() * WALK_SPEED * time.delta_secs();

    // Pas de corps physique pour le joueur : on le garde derrière la ligne, entre les bureaux.
    let p = &mut transform.translation;
    p.x = p.x.clamp(-ROOM_HALF_X + 1.1, ROOM_HALF_X - 1.1);
    p.z = p.z.clamp(THROW_LINE_Z + 0.05, ROOM_HALF_Z - 0.4);
}

/// Ce que fait le pointeur d'un geste cette image : premier doigt, sinon souris non capturée.
struct PointerFrame {
    pressed: Option<Vec2>,
    held: Option<Vec2>,
    released: bool,
}

fn read_pointer(touches: &Touches, mouse: &ButtonInput<MouseButton>, window: &Window) -> PointerFrame {
    let touch_active = touches.iter().next().is_some()
        || touches.iter_just_released().next().is_some()
        || touches.iter_just_canceled().next().is_some();
    if touch_active {
        return PointerFrame {
            pressed: touches.iter_just_pressed().next().map(|touch| touch.position()),
            held: touches.iter().next().map(|touch| touch.position()),
            released: touches.iter_just_released().next().is_some()
                || touches.iter_just_canceled().next().is_some(),
        };
    }
    let cursor = window.cursor_position();
    PointerFrame {
        pressed: cursor.filter(|_| mouse.just_pressed(MouseButton::Left)),
        held: cursor.filter(|_| mouse.pressed(MouseButton::Left)),
        released: mouse.just_released(MouseButton::Left),
    }
}

type Bodies<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut LinearVelocity,
        &'static mut AngularVelocity,
        &'static mut GravityScale,
    ),
>;

fn grab(hand: &mut Hand, bodies: &mut Bodies, entity: Entity, armed: bool) {
    *hand = Hand {
        held: Some(entity),
        armed,
        ..default()
    };
    if let Ok((_, _, mut gravity)) = bodies.get_mut(entity) {
        gravity.0 = 0.0;
    }
}

fn drop_held(hand: &mut Hand, bodies: &mut Bodies) {
    if let Some(entity) = hand.held
        && let Ok((mut velocity, _, mut gravity)) = bodies.get_mut(entity)
    {
        velocity.0 = Vec3::ZERO;
        gravity.0 = 1.0;
    }
    *hand = Hand::default();
}

fn throw_held(hand: &mut Hand, bodies: &mut Bodies, camera: &Transform, now: f32) {
    let Some(start) = hand.charge_start else { return };
    if let Some(entity) = hand.held
        && let Ok((mut velocity, mut spin, mut gravity)) = bodies.get_mut(entity)
    {
        let power = ((now - start) / FULL_CHARGE_SECS).clamp(0.0, 1.0);
        velocity.0 = camera.forward() * (MIN_THROW_SPEED + (MAX_THROW_SPEED - MIN_THROW_SPEED) * power);
        spin.0 = camera.right() * -6.0;
        gravity.0 = 1.0;
    }
    *hand = Hand::default();
}

fn handle_pointer(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    window: Single<&Window>,
    cursor: Single<&CursorOptions>,
    camera: Single<(&mut Player, &Transform, &Camera, &GlobalTransform)>,
    trash: Query<(Entity, &Transform), (With<Trash>, Without<Player>)>,
    mut bodies: Bodies,
    mut hand: ResMut<Hand>,
    mut gesture: ResMut<Gesture>,
    mut game: ResMut<Game>,
    mut crosshair: Single<&mut BackgroundColor, With<Crosshair>>,
) {
    let (mut player, transform, camera, camera_transform) = camera.into_inner();
    let now = time.elapsed_secs();
    // Déchet à portée le plus proche d'un point de l'écran.
    let near = |point: Vec2, tolerance: f32| {
        trash
            .iter()
            .filter(|(_, item)| item.translation.distance(transform.translation) < REACH)
            .filter_map(|(entity, item)| {
                let on_screen = camera.world_to_viewport(camera_transform, item.translation).ok()?;
                Some((entity, on_screen.distance(point)))
            })
            .filter(|(_, distance)| *distance < tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(entity, _)| entity)
    };
    let center = window.size() / 2.0;
    let aimed = || near(center, CROSSHAIR_TOLERANCE);
    crosshair.0 = if hand.held.is_none() && aimed().is_some() {
        CROSSHAIR_TARGET
    } else {
        CROSSHAIR_IDLE
    };

    if pointer_locked(&cursor) {
        if game.finished_at.is_some() && mouse.just_pressed(MouseButton::Left) {
            game.restart_requested = true;
        }
        match hand.held {
            None => {
                if mouse.just_pressed(MouseButton::Left)
                    && let Some(entity) = aimed()
                {
                    grab(&mut hand, &mut bodies, entity, false);
                }
            }
            Some(_) if !hand.armed => hand.armed = !mouse.pressed(MouseButton::Left),
            Some(_) => {
                if mouse.just_pressed(MouseButton::Right) {
                    drop_held(&mut hand, &mut bodies);
                } else if mouse.just_pressed(MouseButton::Left) {
                    hand.charge_start = Some(now);
                } else if mouse.just_released(MouseButton::Left) {
                    throw_held(&mut hand, &mut bodies, transform, now);
                }
            }
        }
        return;
    }

    // Geste : glisser pour regarder, toucher pour ramasser, appui long puis relâcher pour lancer.
    let pointer = read_pointer(&touches, &mouse, &window);
    if let Some(position) = pointer.pressed {
        *gesture = Gesture {
            start: Some(position),
            last: position,
            dragging: false,
        };
        if hand.held.is_some() {
            hand.charge_start = Some(now);
        }
    }
    let Some(start) = gesture.start else { return };
    if let Some(position) = pointer.held {
        if !gesture.dragging && position.distance(start) > DRAG_THRESHOLD {
            gesture.dragging = true;
            hand.charge_start = None;
        }
        if gesture.dragging {
            let delta = position - gesture.last;
            player.yaw -= delta.x * DRAG_SENSITIVITY;
            player.pitch = (player.pitch - delta.y * DRAG_SENSITIVITY).clamp(-1.5, 1.5);
        }
        gesture.last = position;
    }
    if pointer.released {
        if !gesture.dragging {
            if game.finished_at.is_some() {
                game.restart_requested = true;
            } else if hand.held.is_some() {
                throw_held(&mut hand, &mut bodies, transform, now);
            } else if let scale = pointer_scale(&window)
                && let Some(entity) = near(gesture.last * scale, TOUCH_TOLERANCE * scale.x)
            {
                grab(&mut hand, &mut bodies, entity, true);
            }
        }
        gesture.start = None;
    }
}

/// L'objet tenu est tiré vers un point devant la caméra : il reste soumis aux collisions.
fn carry_held(
    mut hand: ResMut<Hand>,
    camera: Single<&Transform, With<Player>>,
    mut bodies: Query<
        (&Transform, &mut LinearVelocity, &mut AngularVelocity, &mut GravityScale),
        Without<Player>,
    >,
) {
    let Some(entity) = hand.held else { return };
    let Ok((transform, mut velocity, mut spin, mut gravity)) = bodies.get_mut(entity) else {
        *hand = Hand::default();
        return;
    };

    let target = camera.translation + camera.forward() * 0.75 - camera.up() * 0.18;
    let offset = target - transform.translation;
    if offset.length() < 0.3 {
        hand.arrived = true;
    }
    if hand.arrived && offset.length() > 1.5 {
        // Coincé derrière un obstacle : on le lâche.
        gravity.0 = 1.0;
        *hand = Hand::default();
        return;
    }
    velocity.0 = (offset * 18.0).clamp_length_max(CARRY_MAX_SPEED);
    spin.0 *= 0.8;
}

fn detect_in_bin(
    mut commands: Commands,
    time: Res<Time>,
    hand: Res<Hand>,
    mut game: ResMut<Game>,
    trash: Query<(Entity, &Transform), With<Trash>>,
) {
    let now = time.elapsed_secs();
    for (entity, transform) in &trash {
        if hand.held == Some(entity) {
            continue;
        }
        let p = transform.translation - BIN_POS;
        if Vec2::new(p.x, p.z).length() < BIN_INNER && p.y > 0.0 && p.y < BIN_HEIGHT {
            commands.entity(entity).remove::<Trash>().insert(InBin);
            game.in_bin += 1;
            game.message_until = now + 1.5;
            if game.in_bin == game.total {
                game.finished_at = Some(now);
            }
        }
    }
}

/// Valide l'étape GeoGaming deux secondes après la fin, le temps de lire le message.
fn notify_geogaming(time: Res<Time>, mut game: ResMut<Game>) {
    let Some(end) = game.finished_at else { return };
    if game.notified || time.elapsed_secs() < end + 2.0 {
        return;
    }
    game.notified = true;
    let seconds = end - game.started_at;
    info!("Bureau propre en {seconds:.1} s : étape validée");
    #[cfg(target_arch = "wasm32")]
    web::notify_completed(seconds);
}

fn restart(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    items: Query<Entity, Or<(With<Trash>, With<InBin>)>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut hand: ResMut<Hand>,
    mut game: ResMut<Game>,
) {
    if !keys.just_pressed(KeyCode::KeyR) && !game.restart_requested {
        return;
    }
    for entity in &items {
        commands.entity(entity).despawn();
    }
    *hand = Hand::default();
    spawn_trash(&mut commands, &mut meshes, &mut materials, &mut game, time.elapsed_secs());
}

// ---------- Interface ----------

fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Crosshair,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            width: Val::Px(8.0),
            height: Val::Px(8.0),
            margin: UiRect {
                left: Val::Px(-4.0),
                top: Val::Px(-4.0),
                ..default()
            },
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(CROSSHAIR_IDLE),
    ));

    commands.spawn((
        ScoreText,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(26.0),
            ..default()
        },
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(20.0),
            top: Val::Px(16.0),
            ..default()
        },
    ));

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Percent(30.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                MessageText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(40.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.2)),
                TextLayout::justify(Justify::Center),
                TextShadow::default(),
            ));
        });

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Px(70.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                PowerBar,
                Node {
                    width: Val::Px(240.0),
                    height: Val::Px(12.0),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                Visibility::Hidden,
            ))
            .with_children(|bar| {
                bar.spawn((
                    PowerFill,
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(1.0, 0.55, 0.15)),
                ));
            });
        });

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Px(20.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(if USE_POINTER_LOCK {
                    "Clic : ramasser | Maintenir puis relâcher : lancer | Clic droit : lâcher | ZQSD : marcher | R : rejouer | Échap : souris"
                } else {
                    "Touchez un déchet pour l'attraper | Glissez pour viser\nMaintenez puis relâchez pour lancer"
                }),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.85)),
                TextLayout::justify(Justify::Center),
                TextShadow::default(),
                Node {
                    max_width: Val::Percent(92.0),
                    ..default()
                },
            ));
        });
}

fn update_ui(
    time: Res<Time>,
    game: Res<Game>,
    hand: Res<Hand>,
    mut score: Single<&mut Text, (With<ScoreText>, Without<MessageText>)>,
    mut message: Single<&mut Text, (With<MessageText>, Without<ScoreText>)>,
    mut bar: Single<&mut Visibility, With<PowerBar>>,
    mut fill: Single<&mut Node, With<PowerFill>>,
) {
    let now = time.elapsed_secs();
    let elapsed = game.finished_at.unwrap_or(now) - game.started_at;
    score.0 = format!(
        "Déchets jetés : {} / {}\nTemps : {:.0} s",
        game.in_bin, game.total, elapsed
    );
    message.0 = if game.finished_at.is_some() {
        let replay = if USE_POINTER_LOCK { "Clic ou R pour rejouer" } else { "Touchez pour rejouer" };
        format!("Bureau propre en {elapsed:.1} s !\n{replay}")
    } else if now < game.message_until {
        "Dans la poubelle !".into()
    } else {
        String::new()
    };

    match hand.charge_start {
        Some(start) => {
            **bar = Visibility::Inherited;
            fill.width = Val::Percent(((now - start) / FULL_CHARGE_SECS).clamp(0.0, 1.0) * 100.0);
        }
        None => **bar = Visibility::Hidden,
    }
}
