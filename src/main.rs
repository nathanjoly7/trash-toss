#[cfg(feature = "autoplay")]
mod autoplay;
mod clean_desk;

use std::f32::consts::{FRAC_PI_2, TAU};

use avian3d::prelude::*;
use bevy::{
    input::{mouse::AccumulatedMouseMotion, touch::Touches},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

/// Natif : vue FPS, souris capturée dès le départ. Web : souris capturée au premier clic dans le
/// jeu (Échap la rend) ; au doigt, on glisse pour regarder et on touche un objet pour le prendre.
const NATIVE: bool = !cfg!(target_arch = "wasm32");
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

/// Au doigt, le tiers gauche de l'écran est un joystick : on y pose le pouce pour marcher.
const JOYSTICK_ZONE: f32 = 0.35;
const JOYSTICK_RADIUS: f32 = 60.0;
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

/// « Trash Toss » par défaut ; « Bureau propre » avec `cargo dev -- bureau-propre`
/// ou `?jeu=bureau-propre` dans l'adresse de la page web.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
enum GameMode {
    Trash,
    CleanDesk,
}

impl GameMode {
    fn detect() -> Self {
        #[cfg(target_arch = "wasm32")]
        let wanted = web::game_param();
        #[cfg(not(target_arch = "wasm32"))]
        let wanted = std::env::args().nth(1).unwrap_or_default();
        if wanted == "bureau-propre" { Self::CleanDesk } else { Self::Trash }
    }

    fn activity_key(self) -> &'static str {
        match self {
            Self::Trash => "trash-toss",
            Self::CleanDesk => "bureau-propre",
        }
    }

    /// Trash Toss : le déchet visé vole jusqu'à la main. Bureau propre : on va jusqu'à l'objet.
    fn reach(self) -> f32 {
        match self {
            Self::Trash => 8.0,
            Self::CleanDesk => 2.5,
        }
    }
}

/// Meuble qu'on ne traverse pas : le joueur n'a pas de corps physique, on le repousse.
#[derive(Component)]
struct Obstacle {
    radius: f32,
}

/// Pouce posé à gauche de l'écran : sa position par rapport au point de départ donne la marche.
#[derive(Resource, Default)]
struct Joystick {
    touch: Option<u64>,
    origin: Vec2,
    current: Vec2,
}

impl Joystick {
    fn input(&self) -> Vec2 {
        if self.touch.is_none() {
            return Vec2::ZERO;
        }
        ((self.current - self.origin) / JOYSTICK_RADIUS).clamp_length_max(1.0)
    }
}

#[derive(Component)]
struct JoystickBase;

#[derive(Component)]
struct JoystickKnob;

/// Tout ce qu'une manche fait apparaître : supprimé puis recréé quand on rejoue.
#[derive(Component)]
struct RoundEntity;

/// Objet qu'on peut attraper et lancer.
#[derive(Component)]
struct Grabbable;

/// Cible qu'on touche sans l'attraper (écran, tableau). Le rayon agrandit la zone sensible.
#[derive(Component)]
struct Touchable {
    radius: f32,
}

/// Cibles touchées cette image, traitées par le mode de jeu.
#[derive(Resource, Default)]
struct Taps(Vec<Entity>);

/// Capture de la souris sur le web : demandée au clic, accordée ou non par le navigateur.
#[derive(Resource, Default)]
struct MouseLook {
    requested_at: Option<f32>,
    /// Demandes restées sans réponse : à trois, on garde le geste (cliquer-glisser pour regarder).
    /// Pas moins : Chrome refuse parfois une nouvelle capture juste après un Échap.
    failures: u32,
}

impl MouseLook {
    fn available(&self) -> bool {
        !NATIVE && self.failures < 3
    }
}

/// Souris et clavier, ou écran tactile : choisit les consignes affichées.
fn uses_mouse() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web::has_fine_pointer()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
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
    done: usize,
    errors: u32,
    points: u32,
    started_at: f32,
    finished_at: Option<f32>,
    message: String,
    message_error: bool,
    message_until: f32,
    notified: bool,
    restart_requested: bool,
    /// Bureau propre sur le web : le joueur lit le débrief, puis touche pour valider l'étape.
    continue_requested: bool,
}

/// Geste en cours (doigt, ou souris non capturée sur le web) : regarder, prendre, lancer.
#[derive(Resource, Default)]
struct Gesture {
    /// Doigt qui fait le geste ; `None` pour la souris.
    touch: Option<u64>,
    /// Posé dans le tiers gauche : s'il glisse, il devient le joystick ; sinon c'est un toucher.
    left_zone: bool,
    start: Option<Vec2>,
    last: Vec2,
    dragging: bool,
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::prelude::*;

    // Contrat des activités Widget GeoGaming : le pont de l'app relaie ce message et valide l'étape.
    // Le bouton « Passer » du cadre envoie le même message avec 0 point.
    #[wasm_bindgen(inline_js = "export function notify_completed(key, seconds, points) { \
        window.parent.postMessage({ completed: true, points: points, activityKey: key, \
        seconds: Math.round(seconds) }, '*'); } \
        export function canvas_css_width() { return document.getElementById('bevy')?.clientWidth ?? 0; } \
        export function canvas_css_height() { return document.getElementById('bevy')?.clientHeight ?? 0; } \
        export function game_param() { return new URLSearchParams(location.search).get('jeu') ?? ''; } \
        export function is_pointer_locked() { return document.pointerLockElement != null; } \
        export function has_fine_pointer() { return window.matchMedia('(pointer: fine)').matches; }")]
    extern "C" {
        pub fn notify_completed(key: &str, seconds: f32, points: u32);
        pub fn canvas_css_width() -> f64;
        pub fn canvas_css_height() -> f64;
        pub fn game_param() -> String;
        pub fn is_pointer_locked() -> bool;
        pub fn has_fine_pointer() -> bool;
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
    let mode = GameMode::detect();
    let trash = |mode: Res<GameMode>| *mode == GameMode::Trash;
    let clean_desk = |mode: Res<GameMode>| *mode == GameMode::CleanDesk;

    let mut app = App::new();
    app.add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: match mode {
                        GameMode::Trash => "Trash Toss",
                        GameMode::CleanDesk => "Bureau propre",
                    }
                    .into(),
                    canvas: Some("#bevy".into()),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            }),
            PhysicsPlugins::default().set(PhysicsInterpolationPlugin::interpolate_all()),
        ))
        .insert_resource(mode)
        .insert_resource(GlobalAmbientLight {
            brightness: 300.0,
            ..default()
        })
        .init_resource::<Hand>()
        .init_resource::<Game>()
        .init_resource::<Gesture>()
        .init_resource::<Joystick>()
        .init_resource::<MouseLook>()
        .init_resource::<Taps>()
        .add_systems(
            Startup,
            (
                use_embedded_font,
                setup_office,
                setup_player,
                setup_ui,
                lock_cursor,
                (setup_bin, setup_trash).run_if(trash),
                clean_desk::setup.after(setup_ui).run_if(clean_desk),
            ),
        )
        .add_systems(
            Update,
            (
                fit_fov,
                look_and_walk,
                handle_pointer,
                carry_held,
                detect_in_bin.run_if(trash),
                (clean_desk::sort_items, clean_desk::apply_taps).run_if(clean_desk),
                notify_geogaming,
                update_joystick_ui,
                update_ui.run_if(trash),
                (clean_desk::update_ui, clean_desk::place_labels).run_if(clean_desk),
                toggle_cursor,
                restart,
            )
                .chain(),
        );
    #[cfg(feature = "autoplay")]
    app.add_systems(Update, (autoplay::autoplay, autoplay::autoplay_clean_desk));
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
    mode: Res<GameMode>,
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

    if *mode == GameMode::Trash {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(2.0 * hx, 0.004, 0.06))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.8, 0.1),
                emissive: LinearRgba::rgb(0.3, 0.24, 0.03),
                ..default()
            })),
            Transform::from_xyz(0.0, 0.002, THROW_LINE_Z),
        ));
    }

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
                shadow_maps_enabled: NATIVE || i == 3,
                ..default()
            },
            Transform::from_xyz(x, h - 0.35, z),
        ));
    }

    // Bureaux contre les murs, écrans tournés vers le centre. Bureau propre : à 18 h, ils sont en
    // veille, sauf la session restée ouverte que le mode ajoute par-dessus.
    let screen = materials.add(match *mode {
        GameMode::Trash => StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::rgb(0.5, 1.0, 2.0),
            ..default()
        },
        GameMode::CleanDesk => StandardMaterial {
            base_color: Color::srgb(0.02, 0.02, 0.03),
            perceptual_roughness: 0.15,
            ..default()
        },
    });
    for z in [-5.0, -2.0, 2.0] {
        spawn_desk(&mut commands, &mut meshes, &mut materials, &screen, Vec3::new(-hx + 0.5, 0.0, z), FRAC_PI_2);
        spawn_desk(&mut commands, &mut meshes, &mut materials, &screen, Vec3::new(hx - 0.5, 0.0, z), -FRAC_PI_2);
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
    screen: &Handle<StandardMaterial>,
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
    let fabric = materials.add(Color::srgb(0.15, 0.2, 0.32));

    // Repère local : plateau le long de x, écran au fond (z négatif) tourné vers +z.
    let parts = [
        (Vec3::new(1.6, 0.04, 0.8), Vec3::new(0.0, 0.74, 0.0), top.clone()),
        (Vec3::new(0.04, 0.72, 0.75), Vec3::new(-0.76, 0.36, 0.0), metal.clone()),
        (Vec3::new(0.04, 0.72, 0.75), Vec3::new(0.76, 0.36, 0.0), metal.clone()),
        (Vec3::new(0.06, 0.25, 0.06), Vec3::new(0.0, 0.885, -0.25), metal.clone()),
        (Vec3::new(0.62, 0.38, 0.03), Vec3::new(0.0, 1.13, -0.25), metal.clone()),
        (Vec3::new(0.58, 0.34, 0.005), Vec3::new(0.0, 1.13, -0.233), screen.clone()),
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
            Grabbable,
            RoundEntity,
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

fn setup_player(mut commands: Commands, mode: Res<GameMode>) {
    commands.spawn((
        Player {
            yaw: 0.0,
            // Trash Toss : les déchets au sol ; Bureau propre : les bacs et la pièce.
            pitch: if *mode == GameMode::Trash { -0.3 } else { -0.15 },
        },
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: 65f32.to_radians(),
            ..default()
        }),
        // Écrans de téléphone très denses : l'anticrénelage y coûte plus qu'il ne rapporte.
        if NATIVE { Msaa::Sample4 } else { Msaa::Off },
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

/// Sur le web, seul le navigateur sait si la souris est capturée (Échap la rend sans prévenir).
fn pointer_locked(cursor: &CursorOptions) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = cursor;
        web::is_pointer_locked()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        cursor.grab_mode != CursorGrabMode::None
    }
}

fn lock_cursor(mut cursor: Single<&mut CursorOptions>) {
    if NATIVE {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

/// Natif : Échap libère la souris, un clic la reprend. Web : un clic demande la capture au
/// navigateur, Échap la rend ; si le navigateur la refuse trois fois, on garde le geste.
fn toggle_cursor(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    mut cursor: Single<&mut CursorOptions>,
    mut mouse_look: ResMut<MouseLook>,
) {
    let clicked = mouse.just_pressed(MouseButton::Left);
    if NATIVE {
        if keys.just_pressed(KeyCode::Escape) {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        } else if clicked && cursor.grab_mode == CursorGrabMode::None {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
        return;
    }

    let now = time.elapsed_secs();
    if pointer_locked(&cursor) {
        *mouse_look = MouseLook::default();
        return;
    }
    match mouse_look.requested_at {
        Some(at) if now - at > 0.8 => {
            mouse_look.requested_at = None;
            mouse_look.failures += 1;
            cursor.grab_mode = CursorGrabMode::None;
        }
        Some(_) => {}
        // Rendue par le navigateur : on se remet d'accord avec lui pour pouvoir la redemander.
        None if cursor.grab_mode != CursorGrabMode::None => cursor.grab_mode = CursorGrabMode::None,
        None if clicked && mouse_look.available() && touches.iter().next().is_none() => {
            cursor.grab_mode = CursorGrabMode::Locked;
            mouse_look.requested_at = Some(now);
        }
        None => {}
    }
}

/// ZQSD en AZERTY (KeyCode suit la position physique) ou flèches.
fn look_and_walk(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions>,
    mode: Res<GameMode>,
    joystick: Res<Joystick>,
    obstacles: Query<(&Transform, &Obstacle), Without<Player>>,
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
    // Joystick : pousser le pouce vers le haut de l'écran fait avancer.
    let stick = joystick.input();
    direction = direction.normalize_or_zero() + forward * -stick.y + right * stick.x;
    transform.translation += direction.clamp_length_max(1.0) * WALK_SPEED * time.delta_secs();

    // Pas de corps physique pour le joueur : on le garde entre les bureaux, et derrière la ligne
    // jaune dans Trash Toss.
    let front = match *mode {
        GameMode::Trash => THROW_LINE_Z + 0.05,
        GameMode::CleanDesk => -ROOM_HALF_Z + 0.4,
    };
    let p = &mut transform.translation;
    p.x = p.x.clamp(-ROOM_HALF_X + 1.1, ROOM_HALF_X - 1.1);
    p.z = p.z.clamp(front, ROOM_HALF_Z - 0.4);
    for (obstacle, Obstacle { radius }) in &obstacles {
        let offset = Vec2::new(p.x - obstacle.translation.x, p.z - obstacle.translation.z);
        if offset.length() < *radius {
            let pushed = offset.normalize_or(Vec2::Y) * *radius;
            p.x = obstacle.translation.x + pushed.x;
            p.z = obstacle.translation.z + pushed.y;
        }
    }
}

/// Ce que fait le pointeur du geste cette image.
#[derive(Default)]
struct PointerFrame {
    /// Doigt (ou `None` pour la souris) et position de l'appui.
    pressed: Option<(Option<u64>, Vec2)>,
    held: Option<Vec2>,
    released: bool,
}

/// Suit le doigt du geste et celui du joystick. Un nouveau doigt devient le geste ; s'il glisse
/// depuis le tiers gauche, `handle_pointer` le passe au joystick. Sans doigt sur l'écran, la
/// souris non capturée fait le geste.
fn read_pointer(
    touches: &Touches,
    mouse: &ButtonInput<MouseButton>,
    window: &Window,
    gesture: &Gesture,
    joystick: &mut Joystick,
) -> PointerFrame {
    let mut pressed = None;
    let mut finger = gesture.touch;
    for touch in touches.iter_just_pressed() {
        if finger.is_none() && gesture.start.is_none() && Some(touch.id()) != joystick.touch {
            finger = Some(touch.id());
            pressed = Some((finger, touch.position()));
        }
    }
    if let Some(id) = joystick.touch {
        match touches.get_pressed(id) {
            Some(touch) => joystick.current = touch.position(),
            None => *joystick = Joystick::default(),
        }
    }

    if let Some(id) = finger {
        return PointerFrame {
            pressed,
            held: touches.get_pressed(id).map(|touch| touch.position()),
            released: touches.just_released(id) || touches.just_canceled(id),
        };
    }
    if touches.iter().next().is_some() || touches.iter_just_released().next().is_some() {
        return PointerFrame::default();
    }
    let cursor = window.cursor_position();
    PointerFrame {
        pressed: cursor.filter(|_| mouse.just_pressed(MouseButton::Left)).map(|p| (None, p)),
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

/// Ce que vise un point de l'écran : un objet à attraper en priorité, sinon une cible à toucher.
#[derive(Clone, Copy)]
enum Target {
    Grab(Entity),
    Touch(Entity),
}

impl Target {
    fn act(self, hand: &mut Hand, bodies: &mut Bodies, taps: &mut Taps, armed: bool) {
        match self {
            Self::Grab(entity) => grab(hand, bodies, entity, armed),
            Self::Touch(entity) => taps.0.push(entity),
        }
    }
}

fn handle_pointer(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<Touches>,
    window: Single<&Window>,
    cursor: Single<&CursorOptions>,
    mode: Res<GameMode>,
    camera: Single<(&mut Player, &Transform, &Camera, &GlobalTransform)>,
    grabbables: Query<(Entity, &Transform), (With<Grabbable>, Without<Player>)>,
    touchables: Query<(Entity, &Transform, &Touchable), Without<Player>>,
    mut bodies: Bodies,
    mut hand: ResMut<Hand>,
    (mut gesture, mut joystick, mouse_look): (ResMut<Gesture>, ResMut<Joystick>, Res<MouseLook>),
    mut game: ResMut<Game>,
    mut taps: ResMut<Taps>,
    mut crosshair: Single<&mut BackgroundColor, With<Crosshair>>,
) {
    let (mut player, transform, camera, camera_transform) = camera.into_inner();
    let now = time.elapsed_secs();
    let closest = |candidates: Vec<(Entity, f32)>| {
        candidates.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)).map(|(entity, _)| entity)
    };
    let target = |point: Vec2, tolerance: f32, reach: f32| {
        let in_reach = |item: &Transform| item.translation.distance(transform.translation) < reach;
        let grab = grabbables
            .iter()
            .filter(|(_, item)| in_reach(item))
            .filter_map(|(entity, item)| {
                let on_screen = camera.world_to_viewport(camera_transform, item.translation).ok()?;
                let distance = on_screen.distance(point);
                (distance < tolerance).then_some((entity, distance))
            })
            .collect();
        // Une cible touchable est sensible sur tout son rayon à l'écran (le tableau fait 3 m).
        let touch = touchables
            .iter()
            .filter(|(_, item, _)| in_reach(item))
            .filter_map(|(entity, item, touchable)| {
                let center = camera.world_to_viewport(camera_transform, item.translation).ok()?;
                let edge = item.translation + camera_transform.right() * touchable.radius;
                let radius = camera.world_to_viewport(camera_transform, edge).ok()?.distance(center);
                let distance = center.distance(point);
                (distance < tolerance.max(radius)).then_some((entity, distance))
            })
            .collect();
        closest(grab).map(Target::Grab).or_else(|| closest(touch).map(Target::Touch))
    };
    // Rien à portée sous le doigt ou le viseur, mais un objet plus loin : on le dit.
    let too_far = |game: &mut Game, point: Vec2, tolerance: f32| {
        if target(point, tolerance, f32::INFINITY).is_some() {
            game.message = "Trop loin : approchez-vous.".into();
            game.message_error = false;
            game.message_until = now + 1.5;
        }
    };
    let center = window.size() / 2.0;
    let aimed = || target(center, CROSSHAIR_TOLERANCE, mode.reach());
    crosshair.0 = if hand.held.is_none() && aimed().is_some() {
        CROSSHAIR_TARGET
    } else {
        CROSSHAIR_IDLE
    };

    // Après la fin : Bureau propre sur le web attend un toucher pour valider l'étape (le temps de
    // lire le débrief) ; sinon on rejoue.
    let after_finish = |game: &mut Game| {
        if *mode == GameMode::CleanDesk && !NATIVE {
            game.continue_requested = true;
        } else {
            game.restart_requested = true;
        }
    };

    if pointer_locked(&cursor) {
        if game.finished_at.is_some() && mouse.just_pressed(MouseButton::Left) {
            after_finish(&mut game);
        }
        match hand.held {
            None if mouse.just_pressed(MouseButton::Left) && game.finished_at.is_none() => {
                match aimed() {
                    Some(target) => target.act(&mut hand, &mut bodies, &mut taps, false),
                    None => too_far(&mut game, center, CROSSHAIR_TOLERANCE),
                }
            }
            None => {}
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
    // Web à la souris : le clic demande la capture (toggle_cursor), il ne prend rien.
    let mouse_click = mouse.just_pressed(MouseButton::Left) && touches.iter().next().is_none();
    if mouse_click && mouse_look.available() && gesture.start.is_none() {
        if game.finished_at.is_some() {
            after_finish(&mut game);
        }
        return;
    }

    let pointer = read_pointer(&touches, &mouse, &window, &gesture, &mut joystick);
    if let Some((touch, position)) = pointer.pressed {
        let css_width = window.width() / pointer_scale(&window).x;
        *gesture = Gesture {
            touch,
            left_zone: touch.is_some() && position.x < css_width * JOYSTICK_ZONE,
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
            hand.charge_start = None;
            if gesture.left_zone && joystick.touch.is_none() {
                // Le pouce glisse depuis la gauche : il marche, depuis l'endroit où il s'est posé.
                *joystick = Joystick {
                    touch: gesture.touch,
                    origin: start,
                    current: position,
                };
                *gesture = Gesture::default();
                return;
            }
            gesture.dragging = true;
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
                after_finish(&mut game);
            } else if hand.held.is_some() {
                throw_held(&mut hand, &mut bodies, transform, now);
            } else {
                let scale = pointer_scale(&window);
                let (point, tolerance) = (gesture.last * scale, TOUCH_TOLERANCE * scale.x);
                match target(point, tolerance, mode.reach()) {
                    Some(target) => target.act(&mut hand, &mut bodies, &mut taps, true),
                    None => too_far(&mut game, point, tolerance),
                }
            }
        }
        gesture.start = None;
        gesture.touch = None;
    }
}

/// Le joystick n'apparaît que sous le pouce, pendant qu'on marche.
fn update_joystick_ui(
    joystick: Res<Joystick>,
    window: Single<&Window>,
    mut base: Single<(&mut Node, &mut Visibility), (With<JoystickBase>, Without<JoystickKnob>)>,
    mut knob: Single<&mut Node, (With<JoystickKnob>, Without<JoystickBase>)>,
) {
    let (node, visibility) = &mut *base;
    if joystick.touch.is_none() {
        **visibility = Visibility::Hidden;
        return;
    }
    **visibility = Visibility::Inherited;
    let scale = pointer_scale(&window);
    let origin = joystick.origin * scale;
    let offset = (joystick.current - joystick.origin).clamp_length_max(JOYSTICK_RADIUS) * scale;
    node.left = Val::Px(origin.x - JOYSTICK_RADIUS);
    node.top = Val::Px(origin.y - JOYSTICK_RADIUS);
    knob.left = Val::Px(JOYSTICK_RADIUS - 24.0 + offset.x);
    knob.top = Val::Px(JOYSTICK_RADIUS - 24.0 + offset.y);
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
            commands.entity(entity).remove::<(Trash, Grabbable)>().insert(InBin);
            game.done += 1;
            game.message = "Dans la poubelle !".into();
            game.message_until = now + 1.5;
            if game.done == game.total {
                game.finished_at = Some(now);
                game.points = 2000;
            }
        }
    }
}

/// Valide l'étape GeoGaming : Trash Toss deux secondes après la fin, le temps de lire le message ;
/// Bureau propre quand le joueur a lu le débrief et touché l'écran.
fn notify_geogaming(time: Res<Time>, mode: Res<GameMode>, mut game: ResMut<Game>) {
    let Some(end) = game.finished_at else { return };
    let ready = match *mode {
        GameMode::Trash => time.elapsed_secs() >= end + 2.0,
        GameMode::CleanDesk => game.continue_requested,
    };
    if game.notified || !ready {
        return;
    }
    game.notified = true;
    let seconds = end - game.started_at;
    info!("{} : terminé en {seconds:.1} s, {} points, étape validée", mode.activity_key(), game.points);
    #[cfg(target_arch = "wasm32")]
    web::notify_completed(mode.activity_key(), seconds, game.points);
}

fn restart(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mode: Res<GameMode>,
    items: Query<Entity, With<RoundEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut hand: ResMut<Hand>,
    mut taps: ResMut<Taps>,
    mut game: ResMut<Game>,
) {
    if !keys.just_pressed(KeyCode::KeyR) && !game.restart_requested {
        return;
    }
    for entity in &items {
        commands.entity(entity).despawn();
    }
    *hand = Hand::default();
    taps.0.clear();
    let now = time.elapsed_secs();
    match *mode {
        GameMode::Trash => spawn_trash(&mut commands, &mut meshes, &mut materials, &mut game, now),
        GameMode::CleanDesk => clean_desk::spawn_round(&mut commands, &mut meshes, &mut materials, &mut game, now),
    }
}

// ---------- Interface ----------

fn setup_ui(mut commands: Commands, mode: Res<GameMode>) {
    let mouse = uses_mouse();
    commands
        .spawn((
            JoystickBase,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(2.0 * JOYSTICK_RADIUS),
                height: Val::Px(2.0 * JOYSTICK_RADIUS),
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.15)),
            Visibility::Hidden,
        ))
        .with_children(|base| {
            base.spawn((
                JoystickKnob,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(48.0),
                    height: Val::Px(48.0),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
            ));
        });

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
                    // Bureau propre explique chaque règle en une phrase : plus petit.
                    font_size: FontSize::Px(if *mode == GameMode::Trash { 40.0 } else { 24.0 }),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.2)),
                TextLayout::justify(Justify::Center),
                TextShadow::default(),
                Node {
                    max_width: Val::Percent(90.0),
                    ..default()
                },
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
                Text::new(match (*mode, NATIVE, mouse) {
                    (GameMode::Trash, true, _) => {
                        "Clic : ramasser | Maintenir puis relâcher : lancer | Clic droit : lâcher | ZQSD : marcher | R : rejouer | Échap : souris"
                    }
                    (GameMode::Trash, false, true) => {
                        "Cliquez dans le jeu pour viser à la souris (Échap pour la libérer)\nClic : ramasser | Maintenir puis relâcher : lancer | ZQSD : avancer"
                    }
                    (GameMode::Trash, false, false) => {
                        "Touchez un déchet : l'attraper | Glissez : viser\nAppui long puis relâcher : lancer\nPouce glissé à gauche : avancer"
                    }
                    (GameMode::CleanDesk, true, _) => {
                        "ZQSD : marcher | Clic sur un objet proche : le prendre ou agir | Maintenir puis relâcher : lancer dans un bac | R : rejouer | Échap : souris"
                    }
                    (GameMode::CleanDesk, false, true) => {
                        "Cliquez dans le jeu pour regarder à la souris (Échap pour la libérer)\nZQSD : marcher | Clic sur un objet proche : le prendre | Maintenir puis relâcher : lancer"
                    }
                    (GameMode::CleanDesk, false, false) => {
                        "Pouce à gauche : marcher | À droite : regarder\nTouchez un objet proche : le prendre\nAppui long puis relâcher : lancer dans un bac"
                    }
                }),
                TextFont {
                    font_size: FontSize::Px(if mouse { 16.0 } else { 14.0 }),
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
        game.done, game.total, elapsed
    );
    message.0 = if game.finished_at.is_some() {
        let replay = if NATIVE { "Clic ou R pour rejouer" } else { "Touchez ou cliquez pour rejouer" };
        format!("Tout est à la poubelle en {elapsed:.1} s !\n{replay}")
    } else if now < game.message_until {
        game.message.clone()
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
