//! Physics world components and management.
//!
//! A [`PhysicsWorld`] entity holds all per-world physics state as components
//! (e.g. [`Gravity`], [`SolverConfig`], [`ColliderTrees`]).
//!
//! The [`MainPhysicsWorld`] marker identifies the default world,
//! spawned automatically by [`PhysicsWorldPlugin`].

use bevy::prelude::*;

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
/// Each converted physics resource is added via `#[require]` so that
/// spawning a `PhysicsWorld` automatically initializes all state.
#[derive(Component, Default)]
#[require(
    Gravity,
    PhysicsLengthUnit,
    SubstepCount,
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

/// Plugin that spawns the [`MainPhysicsWorld`] entity in `build()`.
pub struct PhysicsWorldPlugin;

impl Plugin for PhysicsWorldPlugin {
    fn build(&self, app: &mut App) {
        let entity = app.world_mut().spawn(MainPhysicsWorld).id();
        app.insert_resource(MainPhysicsWorldEntity(entity));
    }
}
