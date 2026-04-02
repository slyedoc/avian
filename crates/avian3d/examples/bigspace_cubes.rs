//! Demonstrates avian physics with two independent reference frames in big_space.
//!
//! Each reference frame is a nested Grid with its own PhysicsWorld.
//! Physics entities in different frames are completely isolated —
//! they have separate collision detection, islands, and solvers.
//!
//! Station A (green, normal gravity) is at the origin.
//! Station B (red, low gravity) is 1 billion units away.
//!
//! Press Tab to teleport between stations.

#![allow(clippy::unnecessary_cast)]

use avian3d::{math::*, prelude::*};
use bevy::prelude::*;
use bevy_math::DVec3;
use big_space::prelude::*;

const STATION_B_DISTANCE: f64 = 1_000_000_000.0;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.build().disable::<TransformPlugin>(),
            BigSpaceDefaultPlugins,
            PhysicsPlugins::default(),
        ))
        .insert_resource(ClearColor(Color::srgb(0.02, 0.02, 0.05)))
        .add_systems(Startup, setup)
        .add_systems(Update, teleport_camera)
        .run();
}

#[derive(Component)]
struct StationLabel;

#[derive(Component)]
struct TeleportState {
    /// Grid entities for each station.
    grids: [Entity; 2],
    /// Which station we're currently at (0 = A, 1 = B).
    current: usize,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cube_mesh = meshes.add(Cuboid::default());
    let ground_mesh = meshes.add(Cuboid::new(20.0, 1.0, 20.0));
    let green_mat = materials.add(Color::srgb(0.2, 0.8, 0.3));
    let red_mat = materials.add(Color::srgb(0.9, 0.3, 0.2));
    let ground_mat = materials.add(Color::srgb(0.5, 0.5, 0.6));

    let mut grid_a = Entity::PLACEHOLDER;
    let mut grid_b = Entity::PLACEHOLDER;

    commands.spawn_big_space_default(|root| {
        let root_grid = root.grid().clone();

        // --- Reference Frame A: at the origin ---
        // A nested Grid that is also a PhysicsWorld.
        let (cell_a, _offset_a) = root_grid.translation_to_grid(DVec3::ZERO);
        root.with_grid(Grid::default(), |frame_a| {
            frame_a.insert((PhysicsWorld, Gravity(Vector::Y * -9.81), cell_a));
            grid_a = frame_a.id();
        });

        // --- Reference Frame B: 1 billion units away ---
        let (cell_b, _offset_b) =
            root_grid.translation_to_grid(DVec3::new(STATION_B_DISTANCE, 0.0, 0.0));
        root.with_grid(Grid::default(), |frame_b| {
            frame_b.insert((PhysicsWorld, Gravity(Vector::Y * -2.0), cell_b));
            grid_b = frame_b.id();
        });

        // Light
        root.spawn_spatial(DirectionalLight {
            illuminance: 5000.0,
            shadow_maps_enabled: true,
            ..default()
        });
    });

    // --- Spawn physics entities with immediate ChildOf ---
    // (bigspace's spawn_spatial defers ChildOf, but avian hooks need it immediately)

    // Station A: green cubes with normal gravity
    spawn_station(
        &mut commands,
        grid_a,
        &cube_mesh,
        &ground_mesh,
        &green_mat,
        &ground_mat,
    );

    // Station B: red cubes with low gravity
    spawn_station(
        &mut commands,
        grid_b,
        &cube_mesh,
        &ground_mesh,
        &red_mat,
        &ground_mat,
    );

    // Camera — starts at Station A
    commands.spawn((
        ChildOf(grid_a),
        Camera3d::default(),
        Transform::from_xyz(0.0, 8.0, 20.0).looking_at(Vec3::new(0.0, 2.0, 0.0), Vec3::Y),
        CellCoord::default(),
        Visibility::default(),
        FloatingOrigin,
        TeleportState {
            grids: [grid_a, grid_b],
            current: 0,
        },
    ));

    // UI
    commands.spawn((
        Text::new(
            "Station A (origin) — green, gravity -9.81\n\
             Press Tab to teleport to Station B",
        ),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
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

fn spawn_station(
    commands: &mut Commands,
    grid_entity: Entity,
    cube_mesh: &Handle<Mesh>,
    ground_mesh: &Handle<Mesh>,
    cube_mat: &Handle<StandardMaterial>,
    ground_mat: &Handle<StandardMaterial>,
) {
    // Ground — all at local origin since each grid is its own reference frame
    commands.spawn((
        ChildOf(grid_entity),
        Mesh3d(ground_mesh.clone()),
        MeshMaterial3d(ground_mat.clone()),
        Transform::from_xyz(0.0, -2.0, 0.0),
        CellCoord::default(),
        Visibility::default(),
        RigidBody::Static,
        Collider::cuboid(20.0, 1.0, 20.0),
    ));

    // Cubes (3x4x3 stack)
    for x in -1..2 {
        for y in 0..4 {
            for z in -1..2 {
                commands.spawn((
                    ChildOf(grid_entity),
                    Mesh3d(cube_mesh.clone()),
                    MeshMaterial3d(cube_mat.clone()),
                    Transform::from_xyz(
                        x as f32 * 1.05,
                        y as f32 * 1.05 + 2.0,
                        z as f32 * 1.05,
                    ),
                    CellCoord::default(),
                    Visibility::default(),
                    RigidBody::Dynamic,
                    Collider::cuboid(1.0, 1.0, 1.0),
                ));
            }
        }
    }
}

fn teleport_camera(
    input: Res<ButtonInput<KeyCode>>,
    mut camera: Query<(Entity, &mut TeleportState), With<Camera3d>>,
    mut label: Query<&mut Text, With<StationLabel>>,
    mut commands: Commands,
) {
    if !input.just_pressed(KeyCode::Tab) {
        return;
    }

    let Ok((cam_entity, mut state)) = camera.single_mut() else {
        return;
    };

    // Toggle station
    state.current = 1 - state.current;
    let target_grid = state.grids[state.current];

    // Re-parent camera to the other grid
    commands.entity(cam_entity).insert(ChildOf(target_grid));

    // Update label
    for mut text in &mut label {
        if state.current == 0 {
            text.0 = "Station A (origin) — green, gravity -9.81\n\
                       Press Tab to teleport to Station B"
                .to_string();
        } else {
            text.0 = "Station B (1 billion units away) — red, gravity -2.0\n\
                       Press Tab to teleport to Station A"
                .to_string();
        }
    }
}
