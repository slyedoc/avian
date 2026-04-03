//! Benchmark: N physics worlds, each with an identical large pyramid.
//!
//! All pyramids occupy the same position but in separate worlds,
//! so there's zero cross-world interaction. This isolates the
//! overhead of running multiple physics worlds.

use avian3d::{math::Scalar, prelude::*};
use bevy::prelude::*;

use super::Benchmark3dPlugins;

/// Creates a single-world baseline benchmark with `pyramid_count` overlapping pyramids.
/// All pyramids share the same world, so entities interact (more broad/narrow phase work).
pub fn create_single_world_bench(pyramid_count: usize, base_count: usize) -> App {
    let mut app = App::new();
    app.add_plugins((Benchmark3dPlugins, PhysicsPlugins::default()));
    super::set_component(&mut app, SubstepCount(4));
    app.add_systems(Startup, move |commands: Commands| {
        setup_single_world(commands, pyramid_count, base_count)
    });
    app
}

fn setup_single_world(mut commands: Commands, pyramid_count: usize, base_count: usize) {
    for _ in 0..pyramid_count {
        spawn_pyramid(&mut commands, None, base_count);
    }
}

/// Creates a benchmark with `world_count` physics worlds, each containing
/// a pyramid with the given `base_count`.
pub fn create_bench(world_count: usize, base_count: usize) -> App {
    let mut app = App::new();
    app.add_plugins((Benchmark3dPlugins, PhysicsPlugins::default()));
    super::set_component(&mut app, SubstepCount(4));
    app.add_systems(Startup, move |commands: Commands| {
        setup(commands, world_count, base_count)
    });
    app
}

fn setup(mut commands: Commands, world_count: usize, base_count: usize) {
    // World 1: the MainPhysicsWorld (auto-spawned by PhysicsWorldPlugin).
    // Spawn a pyramid directly (entities without a PhysicsWorld ancestor use MainPhysicsWorld).
    spawn_pyramid(&mut commands, None, base_count);

    // Worlds 2..N: each gets its own PhysicsWorld entity.
    for i in 1..world_count {
        let world_entity = commands
            .spawn((
                PhysicsWorld,
                Name::new(format!("PhysicsWorld_{i}")),
            ))
            .id();
        spawn_pyramid(&mut commands, Some(world_entity), base_count);
    }
}

fn spawn_pyramid(commands: &mut Commands, physics_world: Option<Entity>, base_count: usize) {
    // Ground
    let mut ground = commands.spawn((
        RigidBody::Static,
        Collider::cuboid(800.0, 40.0, 800.0),
        Transform::from_xyz(0.0, -20.0, 0.0),
    ));
    if let Some(world) = physics_world {
        ground.insert(ChildOf(world));
    }

    // Pyramid
    let h = 0.5;
    let box_size = 2.0 * h;
    let collider = Collider::cuboid(box_size as Scalar, box_size as Scalar, box_size as Scalar);
    let shift = h;
    for i in 0..base_count {
        let y = (2.0 * i as f32 + 1.0) * shift * 0.99;

        for j in i..base_count {
            let x = (i as f32 + 1.0) * shift + 2.0 * (j - i) as f32 * shift
                - h * base_count as f32;

            let mut cube = commands.spawn((
                RigidBody::Dynamic,
                collider.clone(),
                Transform::from_xyz(x, y, 0.0),
            ));
            if let Some(world) = physics_world {
                cube.insert(ChildOf(world));
            }
        }
    }
}
