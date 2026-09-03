//! Demonstrates multiple physics worlds with different gravity
//! and transferring entities between them.
//!
//! The left stack (blue) uses the default `MainPhysicsWorld` with normal gravity.
//! The right stack (orange) uses a second `PhysicsWorld` with reduced gravity.
//!
//! Press Space to transfer a random cube from the left world to the right,
//! or Backspace to transfer one back.

#![allow(clippy::unnecessary_cast)]

use avian3d::{math::*, prelude::*};
use bevy::prelude::*;
use examples_common_3d::ExampleCommonPlugin;
use rand::seq::IteratorRandom;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            ExampleCommonPlugin,
            PhysicsPlugins::default(),
        ))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .add_systems(Startup, setup)
        .add_systems(Update, (transfer_cubes, color_by_sleep_state))
        .run();
}

#[derive(Component)]
struct BlueCube;

#[derive(Component)]
struct OrangeCube;

#[derive(Resource)]
struct SecondWorld(Entity);

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cube_mesh = meshes.add(Cuboid::default());
    let ground_material = materials.add(Color::srgb(0.7, 0.7, 0.8));
    let blue_material = materials.add(Color::srgb(0.3, 0.5, 0.9));
    let orange_material = materials.add(Color::srgb(0.9, 0.5, 0.2));

    let cube_size = 1.0;

    // --- Left stack: default world (normal gravity, blue) ---

    // Ground
    commands.spawn((
        Mesh3d(cube_mesh.clone()),
        MeshMaterial3d(ground_material.clone()),
        Transform::from_xyz(-8.0, -2.0, 0.0).with_scale(Vec3::new(12.0, 1.0, 12.0)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
    ));

    // Blue cubes
    for x in -1..2 {
        for y in 0..4 {
            for z in -1..2 {
                let position = Vec3::new(
                    -8.0 + x as f32 * (cube_size + 0.05),
                    y as f32 * (cube_size + 0.05) + 2.0,
                    z as f32 * (cube_size + 0.05),
                );
                commands.spawn((
                    Mesh3d(cube_mesh.clone()),
                    MeshMaterial3d(blue_material.clone()),
                    Transform::from_translation(position).with_scale(Vec3::splat(cube_size as f32)),
                    RigidBody::Dynamic,
                    Collider::cuboid(1.0, 1.0, 1.0),
                    BlueCube,
                ));
            }
        }
    }

    // --- Right stack: second world (low gravity, orange) ---

    // Spawn a second physics world with low gravity.
    let second_world = commands
        .spawn((
            PhysicsWorld,
            Gravity(Vec3::Y * -2.0),
            Name::new("SecondPhysicsWorld"),
        ))
        .id();

    commands.insert_resource(SecondWorld(second_world));

    // Ground
    commands.spawn((
        ChildOf(second_world),
        Mesh3d(cube_mesh.clone()),
        MeshMaterial3d(ground_material),
        Transform::from_xyz(8.0, -2.0, 0.0).with_scale(Vec3::new(12.0, 1.0, 12.0)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
    ));

    // Orange cubes
    for x in -1..2 {
        for y in 0..4 {
            for z in -1..2 {
                let position = Vec3::new(
                    8.0 + x as f32 * (cube_size + 0.05),
                    y as f32 * (cube_size + 0.05) + 2.0,
                    z as f32 * (cube_size + 0.05),
                );
                commands.spawn((
                    ChildOf(second_world),
                    Mesh3d(cube_mesh.clone()),
                    MeshMaterial3d(orange_material.clone()),
                    Transform::from_translation(position)
                        .with_scale(Vec3::splat(cube_size as f32)),
                    RigidBody::Dynamic,
                    Collider::cuboid(1.0, 1.0, 1.0),
                    OrangeCube,
                ));
            }
        }
    }

    // --- UI ---
    commands.spawn((
        Text::new("Default World (gravity -9.81)\nSpace: transfer blue → orange\nBackspace: transfer orange → blue"),
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
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::default().looking_at(Vec3::new(-1.0, -2.5, -1.5), Vec3::Y),
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 12.0, 35.0))
            .looking_at(Vec3::new(0.0, 3.0, 0.0), Vec3::Y),
    ));
}

fn transfer_cubes(
    input: Res<ButtonInput<KeyCode>>,
    blue_cubes: Query<Entity, With<BlueCube>>,
    orange_cubes: Query<Entity, With<OrangeCube>>,
    second_world: Res<SecondWorld>,
    main_world: Res<MainPhysicsWorldEntity>,
    mut commands: Commands,
) {
    let mut rng = rand::rng();

    // Space: transfer a random blue cube → orange world
    if input.just_pressed(KeyCode::Space) {
        if let Some(entity) = blue_cubes.iter().choose(&mut rng) {
            commands.trigger(TransferToWorld {
                entity,
                world: second_world.0,
            });

            // Place above the right stack and reset velocity.
            commands.entity(entity).insert((
                Transform::from_xyz(8.0, 6.0, 0.0),
                LinearVelocity::ZERO,
                AngularVelocity::ZERO,
            ));

            commands
                .entity(entity)
                .remove::<BlueCube>()
                .insert(OrangeCube);
        }
    }

    // Backspace: transfer a random orange cube → blue world
    if input.just_pressed(KeyCode::Backspace) {
        if let Some(entity) = orange_cubes.iter().choose(&mut rng) {
            commands.trigger(TransferToWorld {
                entity,
                world: main_world.0,
            });

            commands.entity(entity).insert((
                Transform::from_xyz(-8.0, 6.0, 0.0),
                LinearVelocity::ZERO,
                AngularVelocity::ZERO,
            ));

            commands
                .entity(entity)
                .remove::<OrangeCube>()
                .insert(BlueCube);
        }
    }
}

/// Debug: color cubes by sleep state.
/// Bright = awake, dark = sleeping.
fn color_by_sleep_state(
    mut materials: ResMut<Assets<StandardMaterial>>,
    cubes: Query<
        (
            &MeshMaterial3d<StandardMaterial>,
            Has<Sleeping>,
            Has<BlueCube>,
            Has<OrangeCube>,
        ),
        With<RigidBody>,
    >,
) {
    for (mat_handle, is_sleeping, is_blue, is_orange) in &cubes {
        let Some(mut material) = materials.get_mut(mat_handle) else {
            continue;
        };
        if is_sleeping {
            // Dark grey when sleeping
            material.base_color = Color::srgb(0.2, 0.2, 0.2);
        } else if is_blue {
            material.base_color = Color::srgb(0.3, 0.5, 0.9);
        } else if is_orange {
            material.base_color = Color::srgb(0.9, 0.5, 0.2);
        }
    }
}


