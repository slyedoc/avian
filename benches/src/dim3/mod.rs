use core::time::Duration;

use bevy::{
    MinimalPlugins,
    app::{App, Plugin, PluginGroup, PluginGroupBuilder},
    ecs::{component::Component, entity::Entity, query::With},
    time::{Time, TimeUpdateStrategy},
    transform::TransformPlugin,
};

use crate::Benchmark;

mod large_pyramid;
mod many_pyramids;
mod multi_world_pyramids;

/// All benchmarks for `avian3d`.
pub const BENCHMARKS: &[Benchmark] = &[
    Benchmark::new("Large Pyramid 3D", "pyramid", || {
        large_pyramid::create_bench(100)
    }),
    Benchmark::new("Many Pyramids 3D", "many_pyramids", || {
        many_pyramids::create_bench(10, 10, 10)
    }),
    // Single-world baseline: N overlapping pyramids in one world (entities interact).
    Benchmark::new("Single-World 1x Pyramid", "single_world", || {
        multi_world_pyramids::create_single_world_bench(1, 50)
    }),
    Benchmark::new("Single-World 2x Pyramid", "single_world", || {
        multi_world_pyramids::create_single_world_bench(2, 50)
    }),
    Benchmark::new("Single-World 4x Pyramid", "single_world", || {
        multi_world_pyramids::create_single_world_bench(4, 50)
    }),
    Benchmark::new("Single-World 8x Pyramid", "single_world", || {
        multi_world_pyramids::create_single_world_bench(8, 50)
    }),
    // Multi-world: same pyramid in N isolated worlds (no cross-world interaction).
    Benchmark::new("Multi-World 1x Pyramid", "multi_world", || {
        multi_world_pyramids::create_bench(1, 50)
    }),
    Benchmark::new("Multi-World 2x Pyramid", "multi_world", || {
        multi_world_pyramids::create_bench(2, 50)
    }),
    Benchmark::new("Multi-World 4x Pyramid", "multi_world", || {
        multi_world_pyramids::create_bench(4, 50)
    }),
    Benchmark::new("Multi-World 8x Pyramid", "multi_world", || {
        multi_world_pyramids::create_bench(8, 50)
    }),
];

/// A plugin group that includes the minimal set of plugins used for benchmarking `avian3d`.
pub struct Benchmark3dPlugins;

impl PluginGroup for Benchmark3dPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        PluginGroupBuilder::start::<Benchmark3dPlugins>()
            .add_group(MinimalPlugins)
            .add(Benchmark3dCorePlugin)
            .add(TransformPlugin)
    }
}

/// A plugin that sets up the core resources and configuration for benchmarking.
struct Benchmark3dCorePlugin;

impl Plugin for Benchmark3dCorePlugin {
    fn build(&self, app: &mut App) {
        // All benchmarks use a fixed time step of 60 FPS with 4 substeps.
        app.insert_resource(Time::from_hz(60.0));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )));
    }
}

fn set_component<T: Component>(app: &mut App, value: T) {
    let world = app.world_mut();
    let entity = world
        .query_filtered::<Entity, With<T>>()
        .single(world)
        .unwrap();
    world.entity_mut(entity).insert(value);
}
