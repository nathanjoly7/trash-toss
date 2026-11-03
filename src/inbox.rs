//! Mode « Boîte mail piégée » : 9 h, on est assis à son poste et on traite huit mails, dont la
//! moitié sont piégés. L'écran en face ouvre la messagerie ; l'imprimante est à droite, la broyeuse
//! à gauche, la corbeille sous le bureau. Un lien montre sa vraie adresse avant de s'ouvrir ; une
//! pièce jointe piégée lance un rançongiciel, qu'on arrête en débranchant le câble réseau.
use std::f32::consts::FRAC_PI_2;

use bevy::{
    prelude::*,
    text::{LineBreak, Underline},
    ui::UiGlobalTransform,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{
    ContextMenu, Crosshair, Game, MenuChoices, MessageText, NATIVE, Player, PointerCaptured,
    RoundEntity, ScoreText, Taps, Touchable,
    clean_desk::{points, spawn_label},
    pointer_scale, uses_mouse,
};

/// Bureau du joueur : au milieu du mur de droite, l'écran contre le mur (+x).
const DESK: Vec3 = Vec3::new(5.5, 0.0, -2.0);
/// Assis face à l'écran : on regarde le mur, légèrement vers le bas.
pub(crate) const SEAT_YAW: f32 = -FRAC_PI_2;
pub(crate) const SEAT_PITCH: f32 = -0.2;
/// Assis, on tourne la tête jusque-là de chaque côté (l'imprimante, la broyeuse, la pièce).
pub(crate) const SEAT_TURN: f32 = 1.9;
const ZOOM_SECS: f32 = 0.35;
/// Un nouveau mail arrive à cet intervalle, ou tout de suite quand la boîte est vide.
const ARRIVAL_SECS: f32 = 12.0;
const FIRST_BATCH: usize = 3;
/// Après le câble débranché, le service informatique restaure le poste.
const RESTORE_SECS: f32 = 3.5;
const ME: &str = "camille.martin@nordhal.fr";

fn rotation() -> Quat {
    Quat::from_rotation_y(-FRAC_PI_2)
}

/// Repère du bureau : x vers la droite du joueur assis, y vers le haut, z vers lui.
fn desk(local: Vec3) -> Vec3 {
    DESK + rotation() * local
}

pub(crate) fn seat_eye() -> Vec3 {
    desk(Vec3::new(0.0, 1.2, 0.62))
}

// ---------- Les mails ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Action {
    Reply,
    Archive,
    Delete,
    Report,
    Link,
    Attachment,
}

/// Le bon geste règle le mail ; le piège compte une erreur ; l'indice ne compte rien.
#[derive(Clone, Copy)]
enum Verdict {
    Right(&'static str),
    Wrong(&'static str),
    Hint(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    PrintPlanning,
    Ransomware,
}

struct Link {
    label: &'static str,
    url: &'static str,
    verdict: Verdict,
}

struct Attachment {
    name: &'static str,
    size: &'static str,
    /// Bouton du dialogue : « Ouvrir » ou « Imprimer ».
    verb: &'static str,
    verdict: Verdict,
    effect: Option<Effect>,
}

struct Mail {
    from_name: &'static str,
    from_address: &'static str,
    subject: &'static str,
    time: &'static str,
    body: &'static str,
    link: Option<Link>,
    attachment: Option<Attachment>,
    reply: Verdict,
    archive: Verdict,
    delete: Verdict,
    report: Verdict,
}

impl Mail {
    fn verdict(&self, action: Action) -> Option<Verdict> {
        match action {
            Action::Reply => Some(self.reply),
            Action::Archive => Some(self.archive),
            Action::Delete => Some(self.delete),
            Action::Report => Some(self.report),
            Action::Link => self.link.as_ref().map(|link| link.verdict),
            Action::Attachment => self.attachment.as_ref().map(|file| file.verdict),
        }
    }
}

/// Dans l'ordre d'arrivée. Quatre pièges, trois vrais mails, une publicité.
const MAILS: [Mail; 8] = [
    Mail {
        from_name: "Julie Roche",
        from_address: "j.roche@nordhal.fr",
        subject: "Planning de la réunion de jeudi",
        time: "08:52",
        body: "Bonjour Camille,\n\nVoici le planning de la réunion fournisseurs de jeudi. Pouvez-vous l'imprimer pour la salle ?\n\nMerci,\nJulie",
        link: None,
        attachment: Some(Attachment {
            name: "planning-jeudi.pdf",
            size: "84 Ko",
            verb: "Imprimer",
            verdict: Verdict::Right("Planning imprimé : il sort à votre droite, sur l'imprimante."),
            effect: Some(Effect::PrintPlanning),
        }),
        reply: Verdict::Hint("Julie attend le planning imprimé, pas une réponse."),
        archive: Verdict::Hint("Julie vous demande d'imprimer la pièce jointe."),
        delete: Verdict::Wrong("Ce mail était légitime : Julie écrit depuis nordhal.fr et envoie le PDF qu'elle annonce."),
        report: Verdict::Wrong("Fausse alerte : Julie écrit depuis nordhal.fr, et la pièce jointe est le PDF annoncé."),
    },
    Mail {
        from_name: "Service informatique",
        from_address: "support@nordhal-it.com",
        subject: "Votre mot de passe expire aujourd'hui",
        time: "09:03",
        body: "Votre mot de passe Nordhal expire dans 2 heures.\n\nPour le conserver, confirmez-le avant 11 h. Sans action de votre part, votre compte sera bloqué.",
        link: Some(Link {
            label: "Conserver mon mot de passe",
            url: "http://nordhal-it.com.verif-compte.ru/login",
            verdict: Verdict::Wrong("Page piège : elle imite la connexion Nordhal pour voler votre mot de passe. Le vrai service informatique écrit depuis nordhal.fr."),
        }),
        attachment: None,
        reply: Verdict::Wrong("Répondre confirme à l'escroc que votre adresse est active."),
        archive: Verdict::Hint("Classé, il garderait son lien piégé dans votre boîte. Regardez l'adresse de l'expéditeur."),
        delete: Verdict::Hint("Avant de le supprimer, signalez-le : le service informatique pourra le bloquer pour tous."),
        report: Verdict::Right("Signalé. Indices : nordhal-it.com au lieu de nordhal.fr, l'urgence, un lien vers un site en .ru."),
    },
    Mail {
        from_name: "DSI Nordhal",
        from_address: "dsi@nordhal.fr",
        subject: "Coupure du réseau samedi matin",
        time: "09:15",
        body: "Bonjour à tous,\n\nLe réseau sera coupé samedi de 8 h à 12 h pour maintenance. Aucune action n'est demandée de votre part.\n\nLa DSI",
        link: None,
        attachment: None,
        reply: Verdict::Hint("Aucune réponse n'est attendue : c'est une simple information."),
        archive: Verdict::Right("Classé. Mail interne, sans lien ni pièce jointe, rien à faire : rien de suspect."),
        delete: Verdict::Right("Supprimé : c'était une simple information, sans risque."),
        report: Verdict::Wrong("Fausse alerte : expéditeur nordhal.fr, aucun lien, aucune demande. Trop de fausses alertes noient les vraies."),
    },
    Mail {
        from_name: "Bureautik Facturation",
        from_address: "factures@bureautik-paiement.com",
        subject: "Facture n° 2025-118 impayée",
        time: "09:31",
        body: "Bonjour,\n\nSauf erreur de notre part, la facture ci-jointe reste impayée. Merci de régulariser sous 48 h pour éviter des pénalités de retard.\n\nService facturation Bureautik",
        link: None,
        attachment: Some(Attachment {
            name: "Facture_2025-118.pdf.exe",
            size: "412 Ko",
            verb: "Ouvrir",
            verdict: Verdict::Wrong("C'était un programme (.exe) déguisé en PDF : un rançongiciel chiffre vos fichiers. Isolez le poste : débranchez son câble réseau (l'ordinateur est au sol, à gauche de votre chaise)."),
            effect: Some(Effect::Ransomware),
        }),
        reply: Verdict::Wrong("Répondre met l'escroc en confiance : il relancera avec une nouvelle facture."),
        archive: Verdict::Hint("Une facture inattendue ne se classe pas sans vérifier : regardez l'extension de la pièce jointe."),
        delete: Verdict::Hint("Avant de le supprimer, signalez-le : d'autres services ont pu le recevoir."),
        report: Verdict::Right("Signalé. Indices : bureautik-paiement.com, une pièce jointe en .exe déguisée en PDF, la menace de pénalités."),
    },
    Mail {
        from_name: "Microsoft 365",
        from_address: "partage@micros0ft-office.com",
        subject: "Julie Roche a partagé « Budget 2026.xlsx » avec vous",
        time: "10:02",
        body: "Julie Roche vous invite à consulter un document.\n\nBudget 2026.xlsx\n\nCe lien expire dans 24 heures.",
        link: Some(Link {
            label: "Ouvrir le document",
            url: "https://micros0ft-office.com/partage/connexion?doc=budget2026",
            verdict: Verdict::Wrong("La page demandait votre mot de passe Microsoft : un vol d'identifiants. Regardez l'adresse : micros0ft, avec un zéro."),
        }),
        attachment: None,
        reply: Verdict::Wrong("Répondre à une adresse piégée confirme que votre boîte est active."),
        archive: Verdict::Hint("Regardez bien l'adresse de l'expéditeur avant de garder ce partage."),
        delete: Verdict::Hint("Avant de le supprimer, signalez-le : la même campagne vise sûrement vos collègues."),
        report: Verdict::Right("Signalé. Indice : micros0ft-office.com, avec un zéro à la place du o. En cas de doute, demandez à Julie."),
    },
    Mail {
        from_name: "Ressources humaines",
        from_address: "rh@nordhal.fr",
        subject: "Inscription au séminaire de rentrée",
        time: "10:20",
        body: "Bonjour,\n\nLe séminaire de rentrée aura lieu le 14 novembre. Merci de vous inscrire sur l'intranet avant vendredi.\n\nL'équipe RH",
        link: Some(Link {
            label: "S'inscrire au séminaire",
            url: "https://intranet.nordhal.fr/seminaire",
            verdict: Verdict::Right("Inscription enregistrée. Le lien menait bien à intranet.nordhal.fr."),
        }),
        attachment: None,
        reply: Verdict::Hint("Les RH demandent une inscription sur l'intranet, pas une réponse."),
        archive: Verdict::Hint("Les RH attendent votre inscription avant vendredi."),
        delete: Verdict::Hint("Les RH attendent votre inscription avant vendredi."),
        report: Verdict::Wrong("Fausse alerte : expéditeur nordhal.fr, lien vers intranet.nordhal.fr."),
    },
    Mail {
        from_name: "Pierre Durand",
        from_address: "pdg.nordhal@gmail.com",
        subject: "Demande confidentielle",
        time: "10:47",
        body: "Camille,\n\nÊtes-vous à votre bureau ? J'ai besoin que vous traitiez un virement urgent, en toute discrétion. Je suis en réunion : répondez-moi uniquement par mail.\n\nPierre Durand\nPrésident-directeur général",
        link: None,
        attachment: None,
        reply: Verdict::Wrong("C'est la fraude au président : l'escroc attend votre réponse pour réclamer le virement. Un PDG n'écrit pas depuis Gmail."),
        archive: Verdict::Hint("Urgence, secret, adresse Gmail : ce mail ne se classe pas, il se signale."),
        delete: Verdict::Hint("Avant de le supprimer, signalez-le : la direction financière doit être prévenue."),
        report: Verdict::Right("Signalé. Fraude au président : adresse Gmail, urgence, secret demandé. En cas de doute, on rappelle par le standard."),
    },
    Mail {
        from_name: "Fournitures Pro",
        from_address: "offres@fournitures-pro.fr",
        subject: "-30 % sur le papier et les cartouches",
        time: "11:05",
        body: "Profitez de nos offres de novembre sur toutes les fournitures de bureau.\n\nPour ne plus recevoir nos offres, désinscrivez-vous en bas de cette page.",
        link: Some(Link {
            label: "Voir les offres",
            url: "https://www.fournitures-pro.fr/offres-novembre",
            verdict: Verdict::Hint("Rien de dangereux, mais ce n'est pas le moment de faire des achats."),
        }),
        attachment: None,
        reply: Verdict::Hint("Inutile de répondre à une publicité."),
        archive: Verdict::Hint("Une publicité n'a pas besoin d'être gardée."),
        delete: Verdict::Right("Supprimé : une simple publicité, sans risque."),
        report: Verdict::Right("Signalé comme indésirable. C'était une publicité, pas un piège : la supprimer suffisait."),
    },
];

const RULES: &str = "\u{2022} Expéditeur : on lit l'adresse entière, pas seulement le nom (nordhal-it.com n'est pas nordhal.fr).\n\
\u{2022} Lien : on regarde où il mène avant de l'ouvrir (survol à la souris, appui long au doigt).\n\
\u{2022} Pièce jointe inattendue : on ne l'ouvre pas, surtout un .exe, un .zip ou un .html.\n\
\u{2022} Urgence, secret, menace : signes d'arnaque. Un PDG ne demande pas de virement par Gmail.\n\
\u{2022} Mail piégé : on le signale, ça protège aussi les collègues.\n\
\u{2022} Rançongiciel : on débranche le câble réseau, on n'éteint pas, on ne paie pas.";

const TOWER_TITLE: &str = "Votre ordinateur";
const TOWER_CHOICES: [&str; 2] = ["L'éteindre", "Débrancher le câble réseau"];

// ---------- État ----------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Unread,
    Read,
    Done,
}

#[derive(Clone, Copy, PartialEq)]
enum Infection {
    Clean,
    Ransom,
    Restoring(f32),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Confirm {
    Link,
    Attachment,
}

/// Ce qu'un bouton de la messagerie demande (ou l'autoplay).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScreenCommand {
    Focus,
    Close,
    Open(usize),
    Back,
    Ask(Confirm),
    Cancel,
    Confirm,
    Act(usize, Action),
    PayRansom,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tone {
    #[default]
    Info,
    Right,
    Wrong,
}

#[derive(Resource)]
pub(crate) struct Inbox {
    /// Un statut par mail arrivé, dans l'ordre de `MAILS`.
    status: Vec<Status>,
    next_arrival: f32,
    open: Option<usize>,
    confirm: Option<Confirm>,
    infection: Infection,
    tone: Tone,
    pub(crate) queue: Vec<ScreenCommand>,
}

impl Default for Inbox {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl Inbox {
    fn new(now: f32) -> Self {
        Self {
            status: vec![Status::Unread; FIRST_BATCH],
            next_arrival: now + ARRIVAL_SECS,
            open: None,
            confirm: None,
            infection: Infection::Clean,
            tone: Tone::Info,
            queue: Vec::new(),
        }
    }

    pub(crate) fn has_arrived(&self, mail: usize) -> bool {
        mail < self.status.len()
    }

    fn pending(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.status.len()).rev().filter(|&i| self.status[i] != Status::Done)
    }
}

/// Zoom sur l'écran : 0 assis, 1 le nez dessus, messagerie ouverte.
#[derive(Resource, Default)]
pub(crate) struct Zoom {
    focused: bool,
    t: f32,
}

// ---------- Décor ----------

/// L'écran allumé : touché, il ouvre la messagerie.
#[derive(Component)]
pub(crate) struct Monitor {
    normal: Handle<StandardMaterial>,
    ransom: Handle<StandardMaterial>,
}

/// Fenêtres dessinées sur l'écran, cachées par le rançongiciel.
#[derive(Component)]
pub(crate) struct ScreenDetail;

#[derive(Component)]
pub(crate) struct UnreadBadge;

#[derive(Component)]
pub(crate) struct Tower;

/// « Ordinateur » au-dessus de la tour, pendant l'attaque : on la cherche des yeux.
#[derive(Component)]
pub(crate) struct TowerLabel;

/// Le câble réseau, de la tour à la prise murale : caché une fois débranché.
#[derive(Component)]
pub(crate) struct NetworkCable;

/// Feuille qui sort de l'imprimante.
#[derive(Component)]
pub(crate) struct PrintedSheet {
    start: f32,
    slot: f32,
}

#[derive(Component)]
pub(crate) struct DebriefPanel;

#[derive(Component)]
pub(crate) struct DebriefResult;

fn desk_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    size: Vec3,
    local: Vec3,
) -> Entity {
    commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(desk(local)).with_rotation(rotation()),
        ))
        .id()
}

fn glow(materials: &mut Assets<StandardMaterial>, emissive: LinearRgba) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive,
        ..default()
    })
}

pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut game: ResMut<Game>,
    time: Res<Time>,
) {
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.15, 0.17),
        perceptual_roughness: 0.5,
        ..default()
    });
    let grey = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.57, 0.6),
        metallic: 0.3,
        perceptual_roughness: 0.5,
        ..default()
    });
    let white = materials.add(Color::srgb(0.9, 0.9, 0.88));
    let paper = materials.add(Color::srgb(0.97, 0.97, 0.95));

    // Clavier et souris devant l'écran.
    desk_box(&mut commands, &mut meshes, &dark, Vec3::new(0.44, 0.024, 0.15), Vec3::new(0.0, 0.772, 0.06));
    desk_box(&mut commands, &mut meshes, &dark, Vec3::new(0.06, 0.03, 0.1), Vec3::new(0.32, 0.775, 0.08));

    // À droite : l'imprimante sur un caisson, son bac de sortie vers le joueur.
    desk_box(&mut commands, &mut meshes, &grey, Vec3::new(0.6, 0.7, 0.55), Vec3::new(1.3, 0.35, -0.1));
    desk_box(&mut commands, &mut meshes, &white, Vec3::new(0.5, 0.3, 0.42), Vec3::new(1.3, 0.85, -0.15));
    desk_box(&mut commands, &mut meshes, &dark, Vec3::new(0.42, 0.01, 0.3), Vec3::new(1.3, 1.005, -0.18));
    desk_box(&mut commands, &mut meshes, &dark, Vec3::new(0.32, 0.01, 0.16), Vec3::new(1.3, 0.765, 0.13));

    // À gauche : la broyeuse, fente sur le dessus.
    desk_box(&mut commands, &mut meshes, &dark, Vec3::new(0.38, 0.6, 0.3), Vec3::new(-1.55, 0.3, -0.05));
    desk_box(
        &mut commands,
        &mut meshes,
        &materials.add(Color::BLACK),
        Vec3::new(0.28, 0.005, 0.03),
        Vec3::new(-1.55, 0.603, -0.05),
    );

    // Sous le bureau, à droite des jambes : la corbeille, une boulette de papier au fond.
    let basket = desk(Vec3::new(0.5, 0.0, 0.2));
    commands.spawn((
        Mesh3d(meshes.add(Extrusion::new(Annulus::new(0.15, 0.162), 0.32))),
        MeshMaterial3d(grey.clone()),
        Transform::from_translation(basket + Vec3::Y * 0.16).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.162, 0.01))),
        MeshMaterial3d(grey),
        Transform::from_translation(basket + Vec3::Y * 0.005),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.045).mesh().ico(1).unwrap())),
        MeshMaterial3d(paper),
        Transform::from_translation(basket + Vec3::new(0.03, 0.055, -0.02)),
    ));

    // Prise réseau au ras du sol, au mur, dans l'axe de l'ordinateur.
    desk_box(&mut commands, &mut meshes, &white, Vec3::new(0.07, 0.07, 0.01), Vec3::new(-0.97, 0.1, -0.495));

    spawn_ui(&mut commands);
    spawn_round(&mut commands, &mut meshes, &mut materials, &mut game, time.elapsed_secs());
}

fn spawn_ui(commands: &mut Commands) {
    commands
        .spawn((
            DebriefPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(5.0),
                right: Val::Percent(5.0),
                top: Val::Percent(12.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                padding: UiRect::all(Val::Px(20.0)),
                border_radius: BorderRadius::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.09, 0.13, 0.94)),
            Visibility::Hidden,
            GlobalZIndex(6),
        ))
        .with_children(|panel| {
            panel.spawn((Text::new("Boîte mail traitée"), font(26.0)));
            panel.spawn((DebriefResult, Text::new(""), font(19.0), TextColor(Color::srgb(1.0, 0.85, 0.2))));
            panel.spawn((Text::new(RULES), font(16.0), TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9))));
            panel.spawn((
                Text::new(if NATIVE { "Clic ou R pour rejouer" } else { "Touchez ou cliquez pour continuer" }),
                font(16.0),
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.65)),
            ));
        });
}

/// Ce qui change pendant une manche : l'écran, la tour et son câble. Remis à neuf quand on rejoue.
pub(crate) fn spawn_round(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    game: &mut Game,
    now: f32,
) {
    // L'écran : une messagerie claire, bandeau bleu, liste à gauche.
    let normal = glow(materials, LinearRgba::rgb(1.1, 1.12, 1.18));
    let ransom = glow(materials, LinearRgba::rgb(2.4, 0.12, 0.08));
    let blue = glow(materials, LinearRgba::rgb(0.1, 0.35, 1.2));
    let line = glow(materials, LinearRgba::rgb(0.45, 0.47, 0.52));
    let list = glow(materials, LinearRgba::rgb(0.8, 0.83, 0.9));
    let red = glow(materials, LinearRgba::rgb(3.0, 0.2, 0.1));
    let panel = desk_box(commands, meshes, &normal, Vec3::new(0.58, 0.34, 0.002), Vec3::new(0.0, 1.13, -0.229));
    commands.entity(panel).insert((
        RoundEntity,
        Monitor { normal, ransom },
        Touchable { radius: 0.3 },
    ));
    commands.entity(panel).with_children(|screen| {
        let mut detail = |size: Vec2, offset: Vec2, material: &Handle<StandardMaterial>| {
            screen.spawn((
                ScreenDetail,
                Mesh3d(meshes.add(Cuboid::new(size.x, size.y, 0.001))),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(offset.x, offset.y, 0.0015),
            ));
        };
        detail(Vec2::new(0.58, 0.04), Vec2::new(0.0, 0.15), &blue);
        detail(Vec2::new(0.19, 0.28), Vec2::new(-0.19, -0.02), &list);
        for row in 0..5 {
            detail(Vec2::new(0.15, 0.008), Vec2::new(-0.19, 0.09 - row as f32 * 0.05), &line);
        }
        for row in 0..6 {
            let width = if row % 3 == 2 { 0.18 } else { 0.3 };
            detail(Vec2::new(width, 0.008), Vec2::new(0.04 + (width - 0.3) / 2.0, 0.08 - row as f32 * 0.035), &line);
        }
        screen.spawn((
            UnreadBadge,
            Mesh3d(meshes.add(Cuboid::new(0.03, 0.03, 0.001))),
            MeshMaterial3d(red),
            Transform::from_xyz(0.25, 0.15, 0.0025),
        ));
    });

    // La tour, au sol à gauche de la chaise : en retrait, le flanc du bureau la cacherait. Voyant
    // vert en façade.
    let case = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.74, 0.78),
        metallic: 0.3,
        perceptual_roughness: 0.4,
        ..default()
    });
    let tower = desk_box(commands, meshes, &case, Vec3::new(0.19, 0.42, 0.42), Vec3::new(-0.97, 0.21, 0.45));
    commands.entity(tower).insert((RoundEntity, Tower));
    let led = glow(materials, LinearRgba::rgb(0.2, 2.5, 0.4));
    let led = desk_box(commands, meshes, &led, Vec3::new(0.02, 0.02, 0.004), Vec3::new(-0.97, 0.36, 0.662));
    commands.entity(led).insert(RoundEntity);

    // Câble réseau bleu : de l'arrière de la tour à la prise murale, au sol. Débranché, il ne reste
    // que le bout qui sort de la tour.
    let cable = materials.add(Color::srgb(0.1, 0.35, 0.85));
    let segments = [
        (Vec3::new(0.012, 0.012, 0.06), Vec3::new(-0.97, 0.1, 0.21), false),
        (Vec3::new(0.012, 0.012, 0.68), Vec3::new(-0.97, 0.1, -0.15), true),
    ];
    for (size, local, unplugs) in segments {
        let segment = desk_box(commands, meshes, &cable, size, local);
        commands.entity(segment).insert(RoundEntity);
        if unplugs {
            commands.entity(segment).insert(NetworkCable);
        }
    }

    commands.insert_resource(Inbox::new(now));
    commands.insert_resource(Zoom::default());
    let act = if uses_mouse() { "Cliquez" } else { "Touchez" };
    *game = Game {
        total: MAILS.len(),
        started_at: now,
        message: format!("9 h, vous arrivez à votre poste. {act} sur l'écran pour lire vos mails : certains sont piégés."),
        message_until: now + 6.0,
        ..default()
    };
}

fn spawn_sheet(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    now: f32,
    slot: f32,
    header: Color,
) {
    let paper = materials.add(Color::srgb(0.97, 0.97, 0.95));
    let header = materials.add(header);
    let ink = materials.add(Color::srgb(0.3, 0.31, 0.35));
    commands
        .spawn((
            RoundEntity,
            PrintedSheet { start: now, slot },
            Mesh3d(meshes.add(Cuboid::new(0.21, 0.003, 0.297))),
            MeshMaterial3d(paper),
            Transform::from_translation(desk(Vec3::new(1.3, 0.85, -0.2))).with_rotation(rotation()),
        ))
        .with_children(|sheet| {
            sheet.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.17, 0.001, 0.04))),
                MeshMaterial3d(header),
                Transform::from_xyz(0.0, 0.002, -0.1),
            ));
            for z in [-0.05, -0.025, 0.0, 0.025, 0.05, 0.075, 0.1] {
                sheet.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.16, 0.001, 0.007))),
                    MeshMaterial3d(ink.clone()),
                    Transform::from_xyz(0.0, 0.002, z),
                ));
            }
        });
}

// ---------- Règles ----------

fn say(game: &mut Game, inbox: &mut Inbox, now: f32, text: &str, tone: Tone) {
    game.message = text.into();
    game.message_error = tone == Tone::Wrong;
    game.message_until = now + if tone == Tone::Wrong { 6.0 } else { 4.0 };
    inbox.tone = tone;
}

fn mark_done(game: &mut Game, inbox: &mut Inbox, zoom: &mut Zoom, captured: &mut PointerCaptured, mail: usize, now: f32) {
    inbox.status[mail] = Status::Done;
    game.done += 1;
    if game.done == game.total {
        game.finished_at = Some(now);
        game.points = points(game.errors);
        zoom.focused = false;
        captured.0 = false;
        inbox.open = None;
        inbox.confirm = None;
    }
}

/// Un toucher sur l'écran ouvre la messagerie.
pub(crate) fn use_taps(mut taps: ResMut<Taps>, monitors: Query<(), With<Monitor>>, mut inbox: ResMut<Inbox>) {
    for entity in taps.0.drain(..) {
        if monitors.contains(entity) {
            inbox.queue.push(ScreenCommand::Focus);
        }
    }
}

/// Bouton de la messagerie sous un point, en pixels logiques. Dialogue ouvert : seuls ses boutons.
fn button_at(
    point: Vec2,
    modal: bool,
    buttons: &Query<(&ScreenButton, &ComputedNode, &UiGlobalTransform)>,
) -> Option<ScreenCommand> {
    buttons
        .iter()
        .filter(|(button, _, _)| !modal || matches!(button.0, ScreenCommand::Confirm | ScreenCommand::Cancel))
        .find(|(_, node, transform)| node.contains_point(**transform, point / node.inverse_scale_factor()))
        .map(|(button, _, _)| button.0)
}

/// Clic, toucher ou Échap dans la messagerie ouverte.
pub(crate) fn screen_input(
    mouse: Res<ButtonInput<MouseButton>>,
    touches: Res<bevy::input::touch::Touches>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window>,
    zoom: Res<Zoom>,
    mut inbox: ResMut<Inbox>,
    buttons: Query<(&ScreenButton, &ComputedNode, &UiGlobalTransform)>,
) {
    if !zoom.focused || zoom.t < 0.9 {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        let back = if inbox.confirm.is_some() {
            ScreenCommand::Cancel
        } else if inbox.open.is_some() {
            ScreenCommand::Back
        } else {
            ScreenCommand::Close
        };
        inbox.queue.push(back);
        return;
    }
    let press = if mouse.just_pressed(MouseButton::Left) {
        window.cursor_position()
    } else {
        touches.iter_just_pressed().next().map(|touch| touch.position() * pointer_scale(&window))
    };
    if let Some(command) = press.and_then(|point| button_at(point, inbox.confirm.is_some(), &buttons)) {
        inbox.queue.push(command);
    }
}

/// Applique les demandes de la messagerie : zoom, navigation, et le geste choisi sur un mail.
pub(crate) fn run_commands(
    mut commands: Commands,
    time: Res<Time>,
    (mut inbox, mut zoom, mut captured, mut game): (ResMut<Inbox>, ResMut<Zoom>, ResMut<PointerCaptured>, ResMut<Game>),
    mut cursor: Single<&mut CursorOptions>,
    (mut meshes, mut materials): (ResMut<Assets<Mesh>>, ResMut<Assets<StandardMaterial>>),
    towers: Query<Entity, With<Tower>>,
    sheets: Query<(), With<PrintedSheet>>,
) {
    if inbox.queue.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    let queue = std::mem::take(&mut inbox.queue);
    for command in queue {
        if game.finished_at.is_some() {
            break;
        }
        match command {
            ScreenCommand::Focus => {
                zoom.focused = true;
                captured.0 = true;
                inbox.confirm = None;
                // L'accueil et les « nouveau mail » ne servent plus une fois la messagerie ouverte.
                if inbox.tone == Tone::Info {
                    game.message_until = now;
                }
                // La messagerie se lit au curseur : on le rend.
                cursor.grab_mode = CursorGrabMode::None;
                cursor.visible = true;
            }
            ScreenCommand::Close => {
                zoom.focused = false;
                captured.0 = false;
                inbox.open = None;
                inbox.confirm = None;
            }
            ScreenCommand::Open(mail) if inbox.has_arrived(mail) => {
                inbox.open = Some(mail);
                inbox.confirm = None;
                if inbox.status[mail] == Status::Unread {
                    inbox.status[mail] = Status::Read;
                }
            }
            ScreenCommand::Back => {
                inbox.open = None;
                inbox.confirm = None;
            }
            ScreenCommand::Ask(what) => inbox.confirm = Some(what),
            ScreenCommand::Cancel => inbox.confirm = None,
            ScreenCommand::Confirm => {
                if let (Some(mail), Some(what)) = (inbox.open, inbox.confirm.take()) {
                    let action = match what {
                        Confirm::Link => Action::Link,
                        Confirm::Attachment => Action::Attachment,
                    };
                    inbox.queue.push(ScreenCommand::Act(mail, action));
                }
            }
            ScreenCommand::PayRansom => {
                game.errors += 1;
                say(
                    &mut game,
                    &mut inbox,
                    now,
                    "Payer ne garantit pas de récupérer vos fichiers et finance l'attaque suivante. Débranchez le câble réseau.",
                    Tone::Wrong,
                );
            }
            ScreenCommand::Act(mail, action) => {
                if !inbox.has_arrived(mail) || inbox.status[mail] == Status::Done || inbox.infection != Infection::Clean {
                    continue;
                }
                let Some(verdict) = MAILS[mail].verdict(action) else { continue };
                let effect = match action {
                    Action::Attachment => MAILS[mail].attachment.as_ref().and_then(|file| file.effect),
                    _ => None,
                };
                if inbox.status[mail] == Status::Unread {
                    inbox.status[mail] = Status::Read;
                }
                match verdict {
                    Verdict::Right(text) => {
                        say(&mut game, &mut inbox, now, text, Tone::Right);
                        inbox.open = None;
                        if effect == Some(Effect::PrintPlanning) {
                            let slot = sheets.iter().count() as f32;
                            spawn_sheet(&mut commands, &mut meshes, &mut materials, now, slot, Color::srgb(0.15, 0.35, 0.8));
                        }
                        mark_done(&mut game, &mut inbox, &mut zoom, &mut captured, mail, now);
                    }
                    Verdict::Wrong(text) => {
                        game.errors += 1;
                        say(&mut game, &mut inbox, now, text, Tone::Wrong);
                        if effect == Some(Effect::Ransomware) {
                            // L'écran passe au rouge, l'imprimante crache la demande de rançon, et la
                            // tour propose de quoi isoler le poste.
                            inbox.infection = Infection::Ransom;
                            inbox.open = None;
                            let slot = sheets.iter().count() as f32;
                            spawn_sheet(&mut commands, &mut meshes, &mut materials, now, slot, Color::srgb(0.85, 0.1, 0.08));
                            let label = spawn_label(&mut commands, "Ordinateur", desk(Vec3::new(-0.97, 0.62, 0.45)));
                            commands.entity(label).insert((RoundEntity, TowerLabel));
                            for tower in &towers {
                                commands.entity(tower).insert((
                                    Touchable { radius: 0.25 },
                                    MenuChoices {
                                        title: TOWER_TITLE,
                                        labels: TOWER_CHOICES.to_vec(),
                                    },
                                ));
                            }
                        }
                    }
                    Verdict::Hint(text) => say(&mut game, &mut inbox, now, text, Tone::Info),
                }
            }
            ScreenCommand::Open(_) => {}
        }
    }
}

/// Menu de la tour, pendant l'attaque : l'éteindre (erreur) ou débrancher le câble (le bon geste).
pub(crate) fn resolve_menu(
    mut commands: Commands,
    time: Res<Time>,
    mut menu: ResMut<ContextMenu>,
    mut game: ResMut<Game>,
    mut inbox: ResMut<Inbox>,
    towers: Query<(), With<Tower>>,
    labels: Query<Entity, With<TowerLabel>>,
    mut cables: Query<&mut Visibility, With<NetworkCable>>,
) {
    let Some((entity, index)) = menu.chosen else { return };
    if !towers.contains(entity) {
        return;
    }
    menu.chosen = None;
    let now = time.elapsed_secs();
    if index == 0 {
        game.errors += 1;
        say(
            &mut game,
            &mut inbox,
            now,
            "Éteint, le poste perd des traces utiles à l'enquête : on le débranche du réseau, sans l'éteindre.",
            Tone::Wrong,
        );
        return;
    }
    cables.iter_mut().for_each(|mut visibility| *visibility = Visibility::Hidden);
    labels.iter().for_each(|label| commands.entity(label).despawn());
    commands.entity(entity).remove::<(Touchable, MenuChoices)>();
    inbox.infection = Infection::Restoring(now + RESTORE_SECS);
    say(
        &mut game,
        &mut inbox,
        now,
        "Câble débranché : le virus n'atteint plus le réseau. Le service informatique restaure votre poste.",
        Tone::Right,
    );
}

/// Les mails arrivent un par un, tout de suite si la boîte est vide. Rien pendant l'attaque.
pub(crate) fn arrivals(time: Res<Time>, mut inbox: ResMut<Inbox>, mut game: ResMut<Game>, zoom: Res<Zoom>) {
    let now = time.elapsed_secs();
    let ready = inbox.status.len() < MAILS.len()
        && inbox.infection == Infection::Clean
        && game.finished_at.is_none()
        && (now >= inbox.next_arrival || inbox.pending().next().is_none());
    if !ready {
        return;
    }
    let mail = inbox.status.len();
    inbox.status.push(Status::Unread);
    inbox.next_arrival = now + ARRIVAL_SECS;
    // Assis devant la pièce : on prévient. Dans la messagerie, le mail apparaît en haut de la liste.
    if !zoom.focused && now >= game.message_until {
        let text = format!("Nouveau mail : {}", MAILS[mail].subject);
        say(&mut game, &mut inbox, now, &text, Tone::Info);
    }
}

/// Fin de la restauration : câble rebranché, écran propre, le mail piégé supprimé par l'informatique.
pub(crate) fn infection(
    time: Res<Time>,
    mut inbox: ResMut<Inbox>,
    (mut game, mut zoom, mut captured): (ResMut<Game>, ResMut<Zoom>, ResMut<PointerCaptured>),
    mut cables: Query<&mut Visibility, With<NetworkCable>>,
) {
    let now = time.elapsed_secs();
    let Infection::Restoring(until) = inbox.infection else { return };
    if now < until {
        return;
    }
    inbox.infection = Infection::Clean;
    cables.iter_mut().for_each(|mut visibility| *visibility = Visibility::Inherited);
    say(
        &mut game,
        &mut inbox,
        now,
        "Poste restauré depuis la sauvegarde. Le service informatique a supprimé le mail piégé.",
        Tone::Info,
    );
    let infected = MAILS
        .iter()
        .position(|mail| mail.attachment.as_ref().is_some_and(|file| file.effect == Some(Effect::Ransomware)));
    if let Some(mail) = infected
        && inbox.status[mail] != Status::Done
    {
        mark_done(&mut game, &mut inbox, &mut zoom, &mut captured, mail, now);
    }
}

/// Une feuille glisse hors de l'imprimante en une seconde et demie, puis s'empile dans le bac.
pub(crate) fn animate_printing(time: Res<Time>, mut sheets: Query<(&PrintedSheet, &mut Transform)>) {
    let now = time.elapsed_secs();
    for (sheet, mut transform) in &mut sheets {
        let t = ((now - sheet.start) / 1.5).clamp(0.0, 1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        let start = Vec3::new(1.3, 0.85, -0.2);
        let end = Vec3::new(1.3, 0.773 + sheet.slot * 0.004, 0.12);
        transform.translation = desk(start.lerp(end, eased));
    }
}

/// Zoom vers l'écran, ou retour à la place assise. `look_and_walk` vient de poser la vue assise.
pub(crate) fn zoom_camera(time: Res<Time>, mut zoom: ResMut<Zoom>, mut camera: Single<&mut Transform, With<Player>>) {
    let goal = if zoom.focused { 1.0 } else { 0.0 };
    if zoom.t != goal {
        let step = time.delta_secs() / ZOOM_SECS;
        zoom.t = (zoom.t + (goal - zoom.t).clamp(-step, step)).clamp(0.0, 1.0);
    }
    if zoom.t <= 0.0 {
        return;
    }
    let eased = zoom.t * zoom.t * (3.0 - 2.0 * zoom.t);
    let close = desk(Vec3::new(0.0, 1.13, 0.3));
    let facing = Quat::from_euler(EulerRot::YXZ, SEAT_YAW, 0.0, 0.0);
    camera.translation = camera.translation.lerp(close, eased);
    camera.rotation = camera.rotation.slerp(facing, eased);
}

// ---------- La messagerie à l'écran ----------

const PAGE: Color = Color::srgb(0.96, 0.965, 0.975);
const INK: Color = Color::srgb(0.12, 0.14, 0.18);
const MUTED: Color = Color::srgb(0.42, 0.45, 0.5);
const ACCENT: Color = Color::srgb(0.1, 0.4, 0.85);
const TITLE_BAR: Color = Color::srgb(0.12, 0.23, 0.37);
const ROW: Color = Color::WHITE;
const ROW_HOVER: Color = Color::srgb(0.9, 0.94, 1.0);
const TOOL: Color = Color::srgb(0.87, 0.89, 0.92);
const TOOL_HOVER: Color = Color::srgb(0.78, 0.82, 0.88);
const REPORT: Color = Color::srgb(0.9, 0.4, 0.1);
const REPORT_HOVER: Color = Color::srgb(0.98, 0.5, 0.15);

#[derive(Component)]
pub(crate) struct ScreenButton(ScreenCommand);

/// Fond au repos et sous la souris.
#[derive(Component)]
pub(crate) struct ButtonColors {
    idle: Color,
    hover: Color,
}

#[derive(Component)]
pub(crate) struct OverlayRoot;

#[derive(Component)]
pub(crate) struct StatusText;

#[derive(Component)]
pub(crate) struct FeedbackBar;

#[derive(Component)]
pub(crate) struct FeedbackText;

fn font(size: f32) -> TextFont {
    TextFont {
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn button(
    parent: &mut ChildSpawnerCommands,
    command: ScreenCommand,
    label: &str,
    (idle, hover, text): (Color, Color, Color),
    size: f32,
) {
    parent.spawn((
        ScreenButton(command),
        ButtonColors { idle, hover },
        Node {
            padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(idle),
        children![(Text::new(label), font(size), TextColor(text))],
    ));
}

/// Reconstruit la messagerie quand son état change ; `update_screen` gère le survol et les messages.
pub(crate) fn sync_screen_ui(mut commands: Commands, inbox: Res<Inbox>, roots: Query<Entity, With<OverlayRoot>>) {
    if !inbox.is_changed() {
        return;
    }
    roots.iter().for_each(|root| commands.entity(root).despawn());
    commands
        .spawn((
            OverlayRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.75)),
            GlobalZIndex(5),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(820.0),
                    height: Val::Percent(100.0),
                    max_height: Val::Px(660.0),
                    flex_direction: FlexDirection::Column,
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(PAGE),
            ))
            .with_children(|window| {
                title_bar(window);
                window
                    .spawn((
                        FeedbackBar,
                        Node {
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                            display: Display::None,
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                    ))
                    .with_children(|bar| {
                        bar.spawn((FeedbackText, Text::new(""), font(14.0), TextColor(Color::WHITE)));
                    });
                window
                    .spawn(Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(14.0)),
                        row_gap: Val::Px(10.0),
                        overflow: Overflow::clip(),
                        ..default()
                    })
                    .with_children(|content| match (inbox.infection, inbox.open) {
                        (Infection::Ransom, _) => ransom_view(content),
                        (Infection::Restoring(_), _) => restoring_view(content),
                        (Infection::Clean, Some(mail)) => mail_view(content, mail),
                        (Infection::Clean, None) => list_view(content, &inbox),
                    });
                window
                    .spawn((
                        Node {
                            height: Val::Px(28.0),
                            padding: UiRect::horizontal(Val::Px(12.0)),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.88, 0.9, 0.92)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            StatusText,
                            Text::new(""),
                            font(12.0),
                            TextColor(MUTED),
                            TextLayout::linebreak(LineBreak::AnyCharacter),
                        ));
                    });
            });

            if let (Some(mail), Some(what)) = (inbox.open, inbox.confirm) {
                confirm_dialog(root, mail, what);
            }
        });
}

fn title_bar(window: &mut ChildSpawnerCommands) {
    window
        .spawn((
            Node {
                height: Val::Px(46.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(TITLE_BAR),
        ))
        .with_children(|bar| {
            bar.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_shrink: 1.0,
                ..default()
            })
            .with_children(|names| {
                names.spawn((Text::new("Messagerie Nordhal"), font(14.0)));
                names.spawn((Text::new(ME), font(11.0), TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7))));
            });
            button(
                bar,
                ScreenCommand::Close,
                "Fermer",
                (Color::srgba(1.0, 1.0, 1.0, 0.14), Color::srgba(1.0, 1.0, 1.0, 0.28), Color::WHITE),
                13.0,
            );
        });
}

fn list_view(content: &mut ChildSpawnerCommands, inbox: &Inbox) {
    let unread = inbox.status.iter().filter(|status| **status == Status::Unread).count();
    content.spawn((
        Text::new(match unread {
            0 => "Boîte de réception".to_string(),
            1 => "Boîte de réception · 1 non lu".to_string(),
            n => format!("Boîte de réception · {n} non lus"),
        }),
        font(17.0),
        TextColor(INK),
    ));
    let mut empty = true;
    content
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(Val::Px(8.0)),
                overflow: Overflow::clip(),
                row_gap: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.86, 0.88, 0.9)),
        ))
        .with_children(|list| {
            for index in inbox.pending() {
                empty = false;
                let mail = &MAILS[index];
                let unread = inbox.status[index] == Status::Unread;
                list.spawn((
                    ScreenButton(ScreenCommand::Open(index)),
                    ButtonColors { idle: ROW, hover: ROW_HOVER },
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                    BackgroundColor(ROW),
                ))
                .with_children(|row| {
                    row.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((
                            Text::new(if unread { format!("\u{2022} {}", mail.from_name) } else { mail.from_name.to_string() }),
                            font(15.0),
                            TextColor(if unread { ACCENT } else { INK }),
                        ));
                        line.spawn((Text::new(mail.time), font(12.0), TextColor(MUTED)));
                    });
                    row.spawn((Text::new(mail.subject), font(14.0), TextColor(if unread { INK } else { MUTED })));
                });
            }
        });
    if empty {
        content.spawn((Text::new("Aucun mail en attente."), font(15.0), TextColor(MUTED)));
    }
}

fn mail_view(content: &mut ChildSpawnerCommands, index: usize) {
    let mail = &MAILS[index];
    content
        .spawn(Node {
            justify_content: JustifyContent::FlexStart,
            ..default()
        })
        .with_children(|row| button(row, ScreenCommand::Back, "Retour", (TOOL, TOOL_HOVER, INK), 13.0));
    content.spawn((Text::new(mail.subject), font(19.0), TextColor(INK)));
    content
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|header| {
            header.spawn((
                Text::new(format!("De : {} <{}>", mail.from_name, mail.from_address)),
                font(14.0),
                TextColor(INK),
                TextLayout::linebreak(LineBreak::AnyCharacter),
            ));
            header.spawn((Text::new(format!("À : {ME}   {}", mail.time)), font(12.0), TextColor(MUTED)));
        });
    content.spawn((
        Node {
            height: Val::Px(1.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.84, 0.86, 0.89)),
    ));
    content.spawn((Text::new(mail.body), font(15.0), TextColor(INK)));

    if let Some(link) = &mail.link {
        content
            .spawn(Node {
                justify_content: JustifyContent::FlexStart,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    ScreenButton(ScreenCommand::Ask(Confirm::Link)),
                    ButtonColors { idle: Color::NONE, hover: ROW_HOVER },
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(6.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    children![(Text::new(link.label), font(15.0), TextColor(ACCENT), Underline)],
                ));
            });
    }
    if let Some(file) = &mail.attachment {
        content
            .spawn(Node {
                justify_content: JustifyContent::FlexStart,
                ..default()
            })
            .with_children(|row| {
                row.spawn((
                    ScreenButton(ScreenCommand::Ask(Confirm::Attachment)),
                    ButtonColors { idle: ROW, hover: ROW_HOVER },
                    Node {
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.75, 0.78, 0.82)),
                    BackgroundColor(ROW),
                    children![
                        (Text::new("Pièce jointe"), font(11.0), TextColor(MUTED)),
                        (Text::new(format!("{}  ({})", file.name, file.size)), font(14.0), TextColor(INK)),
                    ],
                ));
            });
    }

    content.spawn(Node {
        flex_grow: 1.0,
        ..default()
    });
    content
        .spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|tools| {
            for (action, label) in [
                (Action::Reply, "Répondre"),
                (Action::Archive, "Classer"),
                (Action::Delete, "Supprimer"),
            ] {
                button(tools, ScreenCommand::Act(index, action), label, (TOOL, TOOL_HOVER, INK), 14.0);
            }
            button(
                tools,
                ScreenCommand::Act(index, Action::Report),
                "Signaler",
                (REPORT, REPORT_HOVER, Color::WHITE),
                14.0,
            );
        });
}

fn ransom_view(content: &mut ChildSpawnerCommands) {
    content
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                padding: UiRect::all(Val::Px(18.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.62, 0.05, 0.05)),
        ))
        .with_children(|screen| {
            screen.spawn((
                Text::new("Vos fichiers ont été chiffrés"),
                font(24.0),
                TextLayout::justify(Justify::Center),
            ));
            screen.spawn((
                Text::new("Tous vos documents sont verrouillés.\nPayez 0,8 bitcoin sous 48 heures pour les récupérer."),
                font(15.0),
                TextLayout::justify(Justify::Center),
            ));
            screen
                .spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|row| {
                    button(
                        row,
                        ScreenCommand::PayRansom,
                        "Payer la rançon",
                        (Color::srgb(0.25, 0.02, 0.02), Color::srgb(0.38, 0.05, 0.05), Color::WHITE),
                        15.0,
                    );
                    button(
                        row,
                        ScreenCommand::Close,
                        "Quitter l'écran",
                        (Color::srgba(1.0, 1.0, 1.0, 0.18), Color::srgba(1.0, 1.0, 1.0, 0.3), Color::WHITE),
                        15.0,
                    );
                });
        });
}

fn restoring_view(content: &mut ChildSpawnerCommands) {
    content
        .spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|screen| {
            screen.spawn((Text::new("Restauration en cours"), font(22.0), TextColor(INK)));
            screen.spawn((
                Text::new("Le service informatique restaure votre poste depuis la sauvegarde."),
                font(15.0),
                TextColor(MUTED),
                TextLayout::justify(Justify::Center),
            ));
        });
}

/// Avant d'ouvrir un lien ou une pièce jointe : où mène le lien, quel fichier on ouvre.
fn confirm_dialog(root: &mut ChildSpawnerCommands, index: usize, what: Confirm) {
    let mail = &MAILS[index];
    let (title, value, verb) = match (what, &mail.link, &mail.attachment) {
        (Confirm::Link, Some(link), _) => ("Ce lien mène à :", link.url.to_string(), "Ouvrir le lien"),
        (Confirm::Attachment, _, Some(file)) => ("Pièce jointe :", format!("{}  ({})", file.name, file.size), file.verb),
        _ => return,
    };
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
    ))
    .with_children(|shade| {
        shade
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    max_width: Val::Px(460.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    border_radius: BorderRadius::all(Val::Px(12.0)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
            ))
            .with_children(|card| {
                card.spawn((Text::new(title), font(14.0), TextColor(MUTED)));
                card.spawn((
                    Text::new(value),
                    font(16.0),
                    TextColor(INK),
                    TextLayout::linebreak(LineBreak::AnyCharacter),
                ));
                card.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::FlexEnd,
                    column_gap: Val::Px(8.0),
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    button(row, ScreenCommand::Cancel, "Annuler", (TOOL, TOOL_HOVER, INK), 14.0);
                    button(row, ScreenCommand::Confirm, verb, (ACCENT, Color::srgb(0.2, 0.5, 0.95), Color::WHITE), 14.0);
                });
            });
    });
}

/// Messagerie : visible une fois le zoom fini ; survol des boutons ; adresse du lien survolé ;
/// message du dernier geste en bandeau.
pub(crate) fn update_screen(
    time: Res<Time>,
    window: Single<&Window>,
    (inbox, zoom, game): (Res<Inbox>, Res<Zoom>, Res<Game>),
    mut root: Query<&mut Visibility, With<OverlayRoot>>,
    mut buttons: Query<(&ScreenButton, &ButtonColors, &ComputedNode, &UiGlobalTransform, &mut BackgroundColor), Without<FeedbackBar>>,
    mut status: Query<&mut Text, (With<StatusText>, Without<FeedbackText>)>,
    mut bar: Query<(&mut Node, &mut BackgroundColor), With<FeedbackBar>>,
    mut feedback: Query<&mut Text, (With<FeedbackText>, Without<StatusText>)>,
) {
    let shown = zoom.focused && zoom.t > 0.85;
    for mut visibility in &mut root {
        *visibility = if shown { Visibility::Inherited } else { Visibility::Hidden };
    }
    if !shown {
        return;
    }

    let cursor = window.cursor_position().filter(|_| uses_mouse());
    let modal = inbox.confirm.is_some();
    let mut hovered = None;
    for (button, colors, node, transform, mut background) in &mut buttons {
        let active = !modal || matches!(button.0, ScreenCommand::Confirm | ScreenCommand::Cancel);
        let over = active
            && cursor.is_some_and(|point| node.contains_point(*transform, point / node.inverse_scale_factor()));
        if over {
            hovered = Some(button.0);
        }
        background.0 = if over { colors.hover } else { colors.idle };
    }

    // Comme un vrai navigateur : l'adresse d'un lien survolé s'affiche en bas.
    let hint = match (hovered, inbox.open) {
        (Some(ScreenCommand::Ask(Confirm::Link)), Some(mail)) => {
            MAILS[mail].link.as_ref().map(|link| link.url.to_string()).unwrap_or_default()
        }
        (_, Some(mail)) if MAILS[mail].link.is_some() && uses_mouse() => "Survolez le lien pour voir où il mène.".into(),
        _ => String::new(),
    };
    for mut text in &mut status {
        if text.0 != hint {
            text.0 = hint.clone();
        }
    }

    let now = time.elapsed_secs();
    let visible = now < game.message_until && !game.message.is_empty();
    for (mut node, mut background) in &mut bar {
        node.display = if visible { Display::Flex } else { Display::None };
        background.0 = match inbox.tone {
            Tone::Right => Color::srgb(0.12, 0.55, 0.3),
            Tone::Wrong => Color::srgb(0.75, 0.16, 0.12),
            Tone::Info => Color::srgb(0.2, 0.32, 0.5),
        };
    }
    for mut text in &mut feedback {
        if visible && text.0 != game.message {
            text.0 = game.message.clone();
        }
    }
}

/// Compteurs, message hors messagerie, débrief, viseur caché pendant la lecture.
pub(crate) fn update_hud(
    time: Res<Time>,
    (game, inbox, zoom): (Res<Game>, Res<Inbox>, Res<Zoom>),
    mut score: Single<&mut Text, (With<ScoreText>, Without<MessageText>, Without<DebriefResult>)>,
    message: Single<(&mut Text, &mut TextColor), (With<MessageText>, Without<ScoreText>, Without<DebriefResult>)>,
    mut result: Single<&mut Text, (With<DebriefResult>, Without<ScoreText>, Without<MessageText>)>,
    mut panel: Single<&mut Visibility, (With<DebriefPanel>, Without<Crosshair>)>,
    mut crosshair: Single<&mut Visibility, (With<Crosshair>, Without<DebriefPanel>)>,
) {
    let now = time.elapsed_secs();
    let elapsed = game.finished_at.unwrap_or(now) - game.started_at;
    score.0 = format!(
        "Mails traités : {} / {}\nErreurs : {}   Temps : {:.0} s",
        game.done, game.total, game.errors, elapsed
    );

    let (mut message, mut color) = message.into_inner();
    message.0 = if game.finished_at.is_none() && !zoom.focused && now < game.message_until {
        game.message.clone()
    } else {
        String::new()
    };
    color.0 = match inbox.tone {
        Tone::Right => Color::srgb(0.55, 1.0, 0.6),
        Tone::Wrong => Color::srgb(1.0, 0.45, 0.35),
        Tone::Info => Color::srgb(1.0, 0.85, 0.2),
    };

    **crosshair = if zoom.t > 0.0 { Visibility::Hidden } else { Visibility::Inherited };

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

/// L'écran du bureau : rouge pendant l'attaque, pastille rouge qui clignote s'il y a du non lu.
pub(crate) fn update_monitor(
    time: Res<Time>,
    inbox: Res<Inbox>,
    mut monitors: Query<(&Monitor, &mut MeshMaterial3d<StandardMaterial>)>,
    mut details: Query<&mut Visibility, (With<ScreenDetail>, Without<UnreadBadge>)>,
    mut badges: Query<&mut Visibility, (With<UnreadBadge>, Without<ScreenDetail>)>,
) {
    let clean = inbox.infection == Infection::Clean;
    for (monitor, mut material) in &mut monitors {
        let wanted = if clean { &monitor.normal } else { &monitor.ransom };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
    for mut visibility in &mut details {
        visibility.set_if_neq(if clean { Visibility::Inherited } else { Visibility::Hidden });
    }
    let unread = inbox.status.contains(&Status::Unread);
    let blink = (time.elapsed_secs() * 4.0).sin() > -0.3;
    for mut visibility in &mut badges {
        visibility.set_if_neq(if clean && unread && blink { Visibility::Inherited } else { Visibility::Hidden });
    }
}
