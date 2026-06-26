//! Physics world components and management.
//!
//! A [`PhysicsWorld`] entity holds all per-world physics state as components
//! (e.g. [`Gravity`], [`SolverConfig`], [`ColliderTrees`]).
//!
//! The [`MainPhysicsWorld`] marker identifies the default world,
//! spawned automatically by [`PhysicsWorldPlugin`].
//!
//! Physics entities are assigned to a world by being descendants of a
//! [`PhysicsWorld`] entity in the hierarchy. Entities without a
//! [`PhysicsWorld`] ancestor fall back to the [`MainPhysicsWorld`].

use bevy::{
    ecs::{relationship::Relationship, system::SystemParam, world::DeferredWorld},
    prelude::*,
};

#[cfg(feature = "bevy_diagnostic")]
use crate::diagnostics::{PhysicsEntityDiagnostics, PhysicsTotalDiagnostics};
#[cfg(feature = "parallel")]
use crate::collision::narrow_phase::system_param::ThreadLocalContactStatusBits;
use crate::{
    collider_tree::{
        optimization::OptimizationTasks,
        update::LastDynamicKinematicAabbUpdate,
        ColliderTreeDiagnostics, ColliderTrees, EnlargedProxies, MovedProxies,
    },
    collision::{
        CollisionDiagnostics,
        narrow_phase::{system_param::ContactStatusBits, NarrowPhaseConfig},
    },
    dynamics::{
        rigid_body::{DefaultFriction, DefaultRestitution},
        solver::{
            constraint_graph::ConstraintGraph,
            islands::{PhysicsIslands, sleeping::AwakeIslandBitVec},
            SolverDiagnostics,
            joint_graph::JointGraph,
            ContactConstraints, ContactSoftnessCoefficients, SolverConfig,
        },
    },
    spatial_query::SpatialQueryDiagnostics,
    prelude::*,
};

/// A physics world entity that holds per-world physics state as components.
///
/// Each per-world component is added via `#[require]` so that
/// spawning a `PhysicsWorld` automatically initializes all state.
///
/// Physics entities are assigned to a world by being descendants
/// of a `PhysicsWorld` entity in the bevy hierarchy. Use
/// [`PhysicsWorldLookup`] to resolve which world an entity belongs to.
#[derive(Component, Default)]
#[require(
    Transform,
    Visibility,
    Gravity,
    PhysicsLengthUnit,
    SolverConfig,
    ContactSoftnessCoefficients,
    NarrowPhaseConfig,
    DefaultFriction,
    DefaultRestitution,
    TimeToSleep,
    ColliderTrees,
    MovedProxies,
    EnlargedProxies,
    ContactGraph,
    JointGraph,
    // TODO: ContactConstraints was previously conditionally initialized via
    // NarrowPhasePlugin::generate_constraints. Now always required.
    // May need a flag to disable constraint generation for sensor-only worlds.
    ContactConstraints,
    ConstraintGraph,
    PhysicsIslands,
    AwakeIslandBitVec,
    ContactStatusBits,
    ColliderTreeOptimization,
    OptimizationTasks,
    LastDynamicKinematicAabbUpdate,
    CollisionDiagnostics,
    SolverDiagnostics,
    ColliderTreeDiagnostics,
    SpatialQueryDiagnostics,
)]
#[cfg_attr(feature = "parallel", require(ThreadLocalContactStatusBits))]
#[cfg_attr(feature = "bevy_diagnostic", require(PhysicsTotalDiagnostics, PhysicsEntityDiagnostics))]
pub struct PhysicsWorld;

/// Marker component for the default physics world.
#[derive(Component, Default)]
#[require(PhysicsWorld)]
pub struct MainPhysicsWorld;

/// Resource holding the entity ID of the [`MainPhysicsWorld`].
#[derive(Resource, Deref)]
pub struct MainPhysicsWorldEntity(pub Entity);

// --- Hierarchy-based world lookup ---

/// Walks up the hierarchy from `entity` to find the nearest ancestor
/// with a [`PhysicsWorld`] component. Works in [`DeferredWorld`] (hooks).
///
/// Returns `None` if no `PhysicsWorld` ancestor is found.
pub fn find_physics_world_in_hierarchy(world: &DeferredWorld, entity: Entity) -> Option<Entity> {
    let mut current = entity;
    loop {
        if world.get::<PhysicsWorld>(current).is_some() {
            return Some(current);
        }
        current = world.get::<ChildOf>(current)?.get();
    }
}

/// Walks up the hierarchy from `entity` to find the nearest ancestor
/// with a [`PhysicsWorld`] component, falling back to [`MainPhysicsWorldEntity`].
///
/// Works in [`DeferredWorld`] (hooks).
pub fn find_physics_world_or_main(world: &DeferredWorld, entity: Entity) -> Entity {
    find_physics_world_in_hierarchy(world, entity)
        .unwrap_or_else(|| world.resource::<MainPhysicsWorldEntity>().0)
}

/// A [`SystemParam`] that resolves which [`PhysicsWorld`] entity a given entity belongs to.
///
/// Walks up the hierarchy via [`ChildOf`] to find the nearest [`PhysicsWorld`] ancestor.
/// Falls back to [`MainPhysicsWorldEntity`] for entities without a `PhysicsWorld` ancestor.
#[derive(SystemParam)]
pub struct PhysicsWorldLookup<'w, 's> {
    parents: Query<'w, 's, &'static ChildOf>,
    worlds: Query<'w, 's, (), With<PhysicsWorld>>,
    main_world: Res<'w, MainPhysicsWorldEntity>,
}

impl PhysicsWorldLookup<'_, '_> {
    /// Returns the [`PhysicsWorld`] entity for the given entity by walking up the hierarchy.
    pub fn world_entity_of(&self, entity: Entity) -> Entity {
        let mut current = entity;
        loop {
            if self.worlds.contains(current) {
                return current;
            }
            match self.parents.get(current) {
                Ok(child_of) => current = child_of.get(),
                Err(_) => return self.main_world.0,
            }
        }
    }
}

/// Plugin that spawns the [`MainPhysicsWorld`] entity.
pub struct PhysicsWorldPlugin;

impl Plugin for PhysicsWorldPlugin {
    fn build(&self, app: &mut App) {
        let entity = app.world_mut().spawn(MainPhysicsWorld).id();
        app.insert_resource(MainPhysicsWorldEntity(entity));
    }
}
