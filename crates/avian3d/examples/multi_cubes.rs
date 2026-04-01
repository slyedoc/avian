//! Demonstrates multiple physics worlds with different gravity.
//!
//! The left stack uses the default `MainPhysicsWorld` (normal gravity).
//! The right stack uses a second `PhysicsWorld` with reduced gravity.
//!
//! NOTE: Per-world iteration is not yet implemented, so both stacks
//! currently share the same physics state. This example is a target
//! for the multi-world feature.

#![allow(clippy::unnecessary_cast)]

use avian3d::{math::*, prelude::*};
use bevy::prelude::*;
use examples_common_3d::ExampleCommonPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            ExampleCommonPlugin,
            PhysicsPlugins::default(),
        ))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .add_systems(Startup, setup)
        .run();
}

/// Marker for entities in the second physics world.
#[derive(Component)]
struct SecondWorld;

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let cube_mesh = meshes.add(Cuboid::default());
    let ground_material = materials.add(Color::srgb(0.7, 0.7, 0.8));
    let blue_material = materials.add(Color::srgb(0.2, 0.7, 0.9));
    let red_material = materials.add(Color::srgb(0.9, 0.3, 0.2));

    // --- Second physics world with low gravity ---
    let _second_world = commands
        .spawn((
            PhysicsWorld,
            Gravity(Vector::Y * -2.0),
            Name::new("SecondPhysicsWorld"),
        ))
        .id();

    // --- Left stack: default world (normal gravity) ---

    // Ground
    commands.spawn((
        Mesh3d(cube_mesh.clone()),
        MeshMaterial3d(ground_material.clone()),
        Transform::from_xyz(-8.0, -2.0, 0.0).with_scale(Vec3::new(12.0, 1.0, 12.0)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
    ));

    // Cubes
    let cube_size = 1.0;
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
                ));
            }
        }
    }

    // --- Right stack: second world (low gravity) ---
    // TODO: Assign these entities to _second_world once PhysicsWorldOf
    // relationship is implemented. For now they fall with normal gravity.

    // Ground
    commands.spawn((
        Mesh3d(cube_mesh.clone()),
        MeshMaterial3d(ground_material),
        Transform::from_xyz(8.0, -2.0, 0.0).with_scale(Vec3::new(12.0, 1.0, 12.0)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
        SecondWorld,
    ));

    // Cubes
    for x in -1..2 {
        for y in 0..4 {
            for z in -1..2 {
                let position = Vec3::new(
                    8.0 + x as f32 * (cube_size + 0.05),
                    y as f32 * (cube_size + 0.05) + 2.0,
                    z as f32 * (cube_size + 0.05),
                );
                commands.spawn((
                    Mesh3d(cube_mesh.clone()),
                    MeshMaterial3d(red_material.clone()),
                    Transform::from_translation(position).with_scale(Vec3::splat(cube_size as f32)),
                    RigidBody::Dynamic,
                    Collider::cuboid(1.0, 1.0, 1.0),
                    SecondWorld,
                ));
            }
        }
    }

    // --- Labels ---
    commands.spawn((
        Text::new("Default World\n(gravity -9.81)"),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.2, 0.7, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            left: Val::Px(80.0),
            ..default()
        },
    ));
    commands.spawn((
        Text::new("Second World\n(gravity -2.0)"),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.3, 0.2)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(40.0),
            right: Val::Px(80.0),
            ..default()
        },
    ));

    // Directional light
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
