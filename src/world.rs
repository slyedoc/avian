//! Physics world components and management.
//!
//! A [`PhysicsWorld`] entity holds all per-world physics state as components
//! (e.g. [`Gravity`], [`SolverConfig`], [`ColliderTrees`]).
//!
//! The [`MainPhysicsWorld`] marker identifies the default world,
//! spawned automatically by [`PhysicsWorldPlugin`].
//!
//! Physics entities are assigned to a world via the [`PhysicsWorldOf`] relationship.
//! New [`RigidBody`] entities are automatically assigned to the [`MainPhysicsWorld`]
//! unless they already have a [`PhysicsWorldOf`] component.

use bevy::{
    ecs::relationship::Relationship,
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
/// Physics entities are assigned to a world via [`PhysicsWorldOf`].
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

// --- Relationship: PhysicsWorldOf / PhysicsWorldMembers ---

/// A [`Relationship`] component that assigns a physics entity to a [`PhysicsWorld`].
///
/// When a [`RigidBody`] is spawned without this component, an observer automatically
/// inserts `PhysicsWorldOf` pointing to the [`MainPhysicsWorld`].
///
/// # Example
///
/// ```ignore
/// // Assign to a specific world:
/// commands.spawn((RigidBody::Dynamic, PhysicsWorldOf { world: my_world_entity }));
///
/// // Or let it default to the main world:
/// commands.spawn(RigidBody::Dynamic); // PhysicsWorldOf auto-assigned
/// ```
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[component(
    immutable,
    on_insert = <PhysicsWorldOf as Relationship>::on_insert,
    on_discard = <PhysicsWorldOf as Relationship>::on_discard,
)]
#[reflect(Debug, Component, PartialEq)]
pub struct PhysicsWorldOf {
    /// The [`Entity`] ID of the [`PhysicsWorld`] this entity belongs to.
    pub world: Entity,
}

impl FromWorld for PhysicsWorldOf {
    fn from_world(_world: &mut World) -> Self {
        PhysicsWorldOf {
            world: Entity::PLACEHOLDER,
        }
    }
}

impl Relationship for PhysicsWorldOf {
    type RelationshipTarget = PhysicsWorldMembers;

    fn get(&self) -> Entity {
        self.world
    }

    fn from(entity: Entity) -> Self {
        PhysicsWorldOf { world: entity }
    }

    fn set_risky(&mut self, entity: Entity) {
        self.world = entity;
    }
}

/// A [`RelationshipTarget`] that collects all entities assigned to a [`PhysicsWorld`]
/// via the [`PhysicsWorldOf`] relationship.
///
/// This component is automatically inserted and populated on [`PhysicsWorld`] entities.
/// Do not modify it directly — instead, set [`PhysicsWorldOf`] on the member entities.
#[derive(Component, Clone, Debug, Default, PartialEq, Reflect)]
#[relationship_target(relationship = PhysicsWorldOf)]
#[reflect(Debug, Component, Default, PartialEq)]
pub struct PhysicsWorldMembers(Vec<Entity>);

impl PhysicsWorldMembers {
    /// Returns the number of members in this physics world.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if there are no members in this physics world.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'a> IntoIterator for &'a PhysicsWorldMembers {
    type Item = <Self::IntoIter as Iterator>::Item;
    type IntoIter = core::iter::Copied<core::slice::Iter<'a, Entity>>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().copied()
    }
}

/// Plugin that spawns the [`MainPhysicsWorld`] entity and sets up
/// auto-assignment of [`PhysicsWorldOf`] for new [`RigidBody`] entities.
pub struct PhysicsWorldPlugin;

impl Plugin for PhysicsWorldPlugin {
    fn build(&self, app: &mut App) {
        let entity = app.world_mut().spawn(MainPhysicsWorld).id();
        app.insert_resource(MainPhysicsWorldEntity(entity));

        // Auto-assign RigidBody entities to the MainPhysicsWorld
        // if they don't already have a PhysicsWorldOf.
        app.add_observer(auto_assign_physics_world);
    }
}

/// Observer that auto-assigns new [`RigidBody`] entities to the [`MainPhysicsWorld`]
/// if they don't already have a [`PhysicsWorldOf`] component.
fn auto_assign_physics_world(
    trigger: On<Insert, RigidBody>,
    query: Query<Has<PhysicsWorldOf>>,
    main_world: Res<MainPhysicsWorldEntity>,
    mut commands: Commands,
) {
    let entity = trigger.event_target();
    if let Ok(has_world) = query.get(entity) {
        if !has_world {
            commands
                .entity(entity)
                .insert(PhysicsWorldOf { world: main_world.0 });
        }
    }
}
