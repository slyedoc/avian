//! avian multi-world physics on bevy_solari's **native floating origin** (no big_space).
//!
//! Two independent [`PhysicsWorld`]s a billion metres apart, each its own isolated
//! simulation with its own [`Gravity`], anchored at a [`SolariGridCell`]. Bodies are
//! children of their world anchor with small *world-local* transforms, so the solver
//! never sees the giant coordinates — solari's GPU floating origin renders each world at
//! its cell. The camera *is* the floating origin; **Tab** teleports the origin between
//! stations, and solari recenters so whichever station you're viewing stays f32-crisp.
//!
//! This is the avian side of the "anchored worlds" foundation: the physics core is
//! solari-agnostic; only this example pulls in `bevy_solari`.

#![allow(clippy::unnecessary_cast)]

use avian3d::{math::*, prelude::*};
use bevy::{
    camera::CameraMainTextureUsages,
    prelude::*,
    render::render_resource::TextureUsages,
    solari::prelude::*,
    solari::transform::{SolariFloatingOrigin, SolariGridCell},
};

/// Metres per grid cell. 1 km cells put Station B's 1e9 m at cell index 1,000,000.
const CELL_EDGE: f32 = 1000.0;
/// Station B is one billion metres from the origin, along +X.
const STATION_B_CELL: i32 = 1_000_000;

fn main() {
    App::new()
        // Every PhysicsWorld is a solari reference frame (the stations' bodies ride
        // their anchor's pose — the GPU frontier re-walks moved subtrees automatically);
        // `AvianSolariPlugin` rebases the camera on a frame handoff.
        .add_plugins((
            DefaultPlugins,
            SolariPlugin,
            PhysicsPlugins::default(),
            AvianSolariPlugin,
        ))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.05)))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                (convert_meshes_to_raytracing, convert_standard_materials_to_solari).chain(),
                teleport,
            ),
        )
        .run();
}

#[derive(Component)]
struct Station {
    /// Origin cells of [A, B]; teleport flips the floating origin between them.
    cells: [SolariGridCell; 2],
    current: usize,
}

#[derive(Component)]
struct StationLabel;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut origin: ResMut<SolariFloatingOrigin>,
) {
    let cube_mesh = meshes.add(Cuboid::default());
    let ground_mesh = meshes.add(Cuboid::new(20.0, 1.0, 20.0));
    let green = materials.add(Color::srgb(0.2, 0.8, 0.3));
    let red = materials.add(Color::srgb(0.9, 0.3, 0.2));
    let ground_mat = materials.add(Color::srgb(0.5, 0.5, 0.6));

    let cell_a = SolariGridCell::new(0, 0, 0);
    let cell_b = SolariGridCell::new(STATION_B_CELL, 0, 0);

    // The camera starts as the floating origin at Station A's cell.
    *origin = SolariFloatingOrigin {
        origin_cell: cell_a.cell,
        cell_edge: CELL_EDGE,
    };

    // Sun.
    commands.spawn((
        SolariDirectionLight {
            illuminance: light_consts::lux::FULL_DAYLIGHT,
            ..default()
        },
        Transform::default().looking_to(Vec3::new(-0.3, -0.8, -0.4), Vec3::Y),
    ));

    // --- Station A: normal gravity, at the origin cell ---
    let station_a = commands
        .spawn((
            Name::new("Station A"),
            PhysicsWorld,
            Gravity(Vector::Y * -9.81),
            cell_a,
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    spawn_station(&mut commands, station_a, &cube_mesh, &ground_mesh, &green, &ground_mat);

    // --- Station B: low gravity, one billion metres away ---
    let station_b = commands
        .spawn((
            Name::new("Station B"),
            PhysicsWorld,
            Gravity(Vector::Y * -2.0),
            cell_b,
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    spawn_station(&mut commands, station_b, &cube_mesh, &ground_mesh, &red, &ground_mat);

    // Camera — the floating origin (no SolariGridCell; its cell lives in the resource).
    // NoGpuGlobalTransformReadback keeps solari from clobbering the camera transform after
    // a recenter (bevy + the recenter own it).
    commands.spawn((
        NoGpuGlobalTransformReadback,
        Camera3d::default(),
        Transform::from_xyz(0.0, 8.0, 22.0).looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
        SolariCamera,
        CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING),
        Msaa::Off,
        Station {
            cells: [cell_a, cell_b],
            current: 0,
        },
    ));

    commands.spawn((
        Text::new(
            "Station A (origin) — green, gravity -9.81\nPress Tab to jump to Station B (1e9 m away)",
        ),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(20.0),
            left: Val::Px(20.0),
            ..default()
        },
        StationLabel,
    ));
}

/// A ground slab + a stack of cubes, all children of `world` with small world-local
/// transforms (so the solver stays in f32-precise local space).
fn spawn_station(
    commands: &mut Commands,
    world: Entity,
    cube_mesh: &Handle<Mesh>,
    ground_mesh: &Handle<Mesh>,
    cube_mat: &Handle<StandardMaterial>,
    ground_mat: &Handle<StandardMaterial>,
) {
    commands.spawn((
        ChildOf(world),
        Mesh3d(ground_mesh.clone()),
        MeshMaterial3d(ground_mat.clone()),
        Transform::from_xyz(0.0, -2.0, 0.0),
        RigidBody::Static,
        Collider::cuboid(20.0, 1.0, 20.0),
    ));

    for x in -1..2 {
        for y in 0..4 {
            for z in -1..2 {
                commands.spawn((
                    ChildOf(world),
                    Mesh3d(cube_mesh.clone()),
                    MeshMaterial3d(cube_mat.clone()),
                    Transform::from_xyz(x as f32 * 1.05, y as f32 * 1.05 + 2.0, z as f32 * 1.05),
                    RigidBody::Dynamic,
                    Collider::cuboid(1.0, 1.0, 1.0),
                ));
            }
        }
    }
}

/// Tab flips the floating origin between the two station cells. solari re-worlds the
/// scene and recenters, so the targeted station renders precisely while the other
/// becomes a distant glint — the physics in both worlds keeps running untouched.
fn teleport(
    input: Res<ButtonInput<KeyCode>>,
    mut origin: ResMut<SolariFloatingOrigin>,
    mut camera: Query<&mut Station>,
    mut label: Query<&mut Text, With<StationLabel>>,
) {
    if !input.just_pressed(KeyCode::Tab) {
        return;
    }
    let Ok(mut station) = camera.single_mut() else {
        return;
    };
    station.current = 1 - station.current;
    origin.origin_cell = station.cells[station.current].cell;

    if let Ok(mut text) = label.single_mut() {
        text.0 = if station.current == 0 {
            "Station A (origin) — green, gravity -9.81\nPress Tab to jump to Station B (1e9 m away)"
                .to_string()
        } else {
            "Station B (1e9 m away) — red, gravity -2.0\nPress Tab to jump to Station A".to_string()
        };
    }
}
