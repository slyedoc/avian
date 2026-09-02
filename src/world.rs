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
use crate::{
    dynamics::joints::joint_graph::JointGraph,
    collider_tree::{
        optimization::OptimizationTasks,
        update::LastDynamicKinematicAabbUpdate,
        ColliderTreeDiagnostics, ColliderTreeProxy, ColliderTreeProxyFlags,
        ColliderTreeProxyKey, ColliderTreeType, ColliderTrees, EnlargedProxies, MovedProxies,
    },
    collision::{
        CollisionDiagnostics,
        narrow_phase::{system_param::ContactStatusBits, NarrowPhaseConfig},
    },
    dynamics::{
        rigid_body::{DefaultFriction, DefaultRestitution},
        solver::{
            constraint_graph::ConstraintGraph,
            islands::{BodyIslandNode, IslandId, PhysicsIslands, sleeping::{AwakeIslandBitVec, WakeIslands}},
            SolverDiagnostics,
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
#[cfg_attr(feature = "bevy_diagnostic", require(PhysicsTotalDiagnostics, PhysicsEntityDiagnostics))]
pub struct PhysicsWorld;

/// Marker component for the default physics world.
#[derive(Component, Default)]
#[require(PhysicsWorld)]
pub struct MainPhysicsWorld;

/// Resource holding the entity ID of the [`MainPhysicsWorld`].
#[derive(Resource, Deref)]
pub struct MainPhysicsWorldEntity(pub Entity);

// --- Cached physics world assignment ---

/// A cached reference to the [`PhysicsWorld`] entity that a physics entity belongs to.
///
/// This is populated automatically by walking the hierarchy on spawn (via the
/// [`BodyIslandNode`] `on_add` hook) and updated directly by [`TransferToWorld`].
///
/// [`PhysicsWorldLookup`] reads this cache for O(1) lookup instead of
/// walking the [`ChildOf`] hierarchy every frame.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct PhysicsWorldEntity(pub Entity);

// --- Hierarchy-based world lookup ---

/// Finds the [`PhysicsWorld`] entity for the given entity.
///
/// Checks the [`PhysicsWorldEntity`] cache first (O(1)), then falls back
/// to walking the [`ChildOf`] hierarchy. Works in [`DeferredWorld`] (hooks).
///
/// Returns `None` if no `PhysicsWorld` is found.
pub fn find_physics_world_in_hierarchy(world: &DeferredWorld, entity: Entity) -> Option<Entity> {
    // Fast path: cached world assignment (set by BodyIslandNode::on_add and TransferToWorld).
    if let Some(cached) = world.get::<PhysicsWorldEntity>(entity) {
        return Some(cached.0);
    }
    // Slow path: walk hierarchy.
    let mut current = entity;
    loop {
        if world.get::<PhysicsWorld>(current).is_some() {
            return Some(current);
        }
        current = world.get::<ChildOf>(current)?.get();
    }
}

/// Finds the [`PhysicsWorld`] entity for the given entity, falling back to
/// [`MainPhysicsWorldEntity`].
///
/// Checks the [`PhysicsWorldEntity`] cache first. Works in [`DeferredWorld`] (hooks).
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
    cache: Query<'w, 's, &'static PhysicsWorldEntity>,
    parents: Query<'w, 's, &'static ChildOf>,
    worlds: Query<'w, 's, (), With<PhysicsWorld>>,
    main_world: Res<'w, MainPhysicsWorldEntity>,
}

impl PhysicsWorldLookup<'_, '_> {
    /// Returns the [`MainPhysicsWorldEntity`], useful when any world's config will do.
    pub fn any_world_entity(&self) -> Entity {
        self.main_world.0
    }

    /// Returns the [`PhysicsWorld`] entity for the given entity.
    ///
    /// Uses the [`PhysicsWorldEntity`] cache if present (O(1)),
    /// otherwise walks up the [`ChildOf`] hierarchy.
    pub fn world_entity_of(&self, entity: Entity) -> Entity {
        // Fast path: cached world assignment.
        if let Ok(cached) = self.cache.get(entity) {
            return cached.0;
        }
        // Slow path: walk hierarchy.
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

// --- Transfer between physics worlds ---

/// An [`EntityEvent`] that transfers a physics entity to a different [`PhysicsWorld`].
///
/// This re-parents the entity and handles all physics cleanup/re-initialization:
/// solver bodies, islands, collider tree proxies, and contacts are properly
/// migrated from the old world to the new one.
///
/// # Reframing
///
/// The body's **velocity** is reframed into the destination frame (subtract the
/// destination frame's velocity, add the source frame's) so a body keeping pace with the
/// source frame arrives keeping pace with the destination — see [`on_transfer_to_world`].
/// Its **pose** keeps the same frame-local `Transform` (correct for co-located frames,
/// e.g. a docked airlock). Full world-position-preserving pose decomposition for
/// spatially-offset frames is a refinement. Completion is announced via [`WorldTransferred`].
///
/// # Example
///
/// ```ignore
/// // Transfer player from ship interior to planet surface
/// commands.trigger(TransferToWorld {
///     entity: player_entity,
///     world: planet_world,
/// });
/// ```
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct TransferToWorld {
    /// The entity to transfer.
    pub entity: Entity,
    /// The [`PhysicsWorld`] entity to transfer to.
    pub world: Entity,
}

/// Fired by [`on_transfer_to_world`] **after** a [`TransferToWorld`] completes — the
/// notification that velocity reframing and any external rebase bind to (e.g. solari's
/// rotational rebase of the previous view-projection when the camera crosses frames).
///
/// Carries **both** frame anchors explicitly, so observers never race the world-cache
/// update inside the transfer handler. Read each anchor's `LinearVelocity`/`AngularVelocity`
/// (and, for a renderer, its world rotation) off the `from`/`to` entities to compute the
/// reframe — the kinematic state lives on the frames, keeping this event pure routing.
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct WorldTransferred {
    /// The entity that was transferred.
    pub entity: Entity,
    /// The [`PhysicsWorld`] it came from.
    pub from: Entity,
    /// The [`PhysicsWorld`] it now belongs to.
    pub to: Entity,
}

/// Handles [`TransferToWorld`] events.
///
/// Directly manipulates physics state via queries — no deferred SolverBody
/// remove/re-add, so no hook ordering issues and no exclusive world lock.
///
/// 1. Removes collider proxy from old world's [`ColliderTrees`]
/// 2. Unlinks body from old world's [`PhysicsIslands`]
/// 3. Re-parents entity via deferred [`ChildOf`] insert
/// 4. Creates a new island in the target world's [`PhysicsIslands`]
/// 5. Re-registers collider proxy via deferred [`ColliderOf`] re-insert
/// 6. Wakes sleeping islands in the target world
#[allow(clippy::type_complexity)]
fn on_transfer_to_world(
    trigger: On<TransferToWorld>,
    mut collider_keys: Query<(
        &mut ColliderTreeProxyKey,
        Option<&RigidBodyColliders>,
        Option<&ColliderOf>,
        &ColliderAabb,
        Option<&CollisionLayers>,
        Has<Sensor>,
        Has<CollisionEventsEnabled>,
        Option<&ActiveCollisionHooks>,
    )>,
    bodies: Query<(&RigidBody, Has<RigidBodyDisabled>)>,
    mut body_islands: Query<&mut BodyIslandNode>,
    mut sleep_timers: Query<&mut crate::dynamics::rigid_body::sleeping::SleepTimer>,
    mut world_cache: Query<&mut PhysicsWorldEntity>,
    parents: Query<&ChildOf>,
    physics_worlds: Query<(), With<PhysicsWorld>>,
    main_world: Res<MainPhysicsWorldEntity>,
    mut world_state: Query<
        (&mut ColliderTrees, &mut MovedProxies, &mut PhysicsIslands, &mut AwakeIslandBitVec),
        With<PhysicsWorld>,
    >,
    // Frame anchors' own motion (kinematic movers like a flying ship); static frames
    // simply lack these. `With`/`Without<PhysicsWorld>` keeps the two velocity queries
    // archetype-disjoint, so the mutable body query doesn't conflict with the frame read.
    frame_vels: Query<(Option<&LinearVelocity>, Option<&AngularVelocity>), With<PhysicsWorld>>,
    mut body_vels: Query<(&mut LinearVelocity, &mut AngularVelocity), Without<PhysicsWorld>>,
    mut commands: Commands,
) {
    let entity = trigger.event_target();
    let target_world = trigger.event().world;

    // Read old world from cache, fallback to hierarchy walk.
    let old_world = world_cache
        .get(entity)
        .map(|c| c.0)
        .unwrap_or_else(|_| {
            let mut current = entity;
            loop {
                if physics_worlds.contains(current) {
                    return current;
                }
                match parents.get(current) {
                    Ok(child_of) => current = child_of.get(),
                    Err(_) => return main_world.0,
                }
            }
        });

    if old_world == target_world {
        return; // Already in the target world.
    }

    // Update the cached world assignment immediately.
    if let Ok(mut cached) = world_cache.get_mut(entity) {
        cached.0 = target_world;
    }

    // --- 1. Remove collider proxy from old world's tree ---
    // Collect child collider entities to avoid borrow conflicts.
    let child_colliders_for_remove: Vec<Entity> = collider_keys
        .get(entity)
        .ok()
        .and_then(|(_, body_colliders, ..)| body_colliders.map(|c| c.iter().collect()))
        .unwrap_or_default();

    remove_proxy_from_world(entity, old_world, &mut collider_keys, &mut world_state);
    for collider in child_colliders_for_remove {
        remove_proxy_from_world(collider, old_world, &mut collider_keys, &mut world_state);
    }

    // --- 2. Unlink body from old world's island ---
    let mut old_island_id = IslandId::PLACEHOLDER;
    let mut old_island_survived = false;

    if let Ok(body_island) = body_islands.get(entity) {
        old_island_id = body_island.island_id;
        let prev = body_island.prev;
        let next = body_island.next;

        // Fix the linked list in the old island.
        if let Some(prev_entity) = prev {
            if let Ok(mut prev_node) = body_islands.get_mut(prev_entity) {
                prev_node.next = next;
            }
        }
        if let Some(next_entity) = next {
            if let Ok(mut next_node) = body_islands.get_mut(next_entity) {
                next_node.prev = prev;
            }
        }

        // Update the old world's island.
        if let Ok((_, _, mut old_islands, _)) = world_state.get_mut(old_world) {
            if let Some(island) = old_islands.get_mut(old_island_id) {
                island.body_count = island.body_count.saturating_sub(1);
                if island.head_body == Some(entity) {
                    island.head_body = next;
                }
                if island.tail_body == Some(entity) {
                    island.tail_body = prev;
                }
                if island.body_count == 0 && island.contact_count == 0 {
                    old_islands.remove_island(old_island_id);
                } else {
                    old_island_survived = true;
                }
            }
        }

        // --- 4. Create a new island in the target world and update the body's node ---
        let mut new_island_id = IslandId::PLACEHOLDER;
        if let Ok((_, _, mut new_islands, mut awake_bits)) = world_state.get_mut(target_world) {
            new_island_id = new_islands.create_island_with(|island| {
                island.head_body = Some(entity);
                island.tail_body = Some(entity);
                island.body_count = 1;
            });
            // Mark the new island as awake so sleep_islands doesn't immediately re-sleep it.
            awake_bits.set_and_grow(new_island_id.0 as usize);
        }
        if let Ok(mut body_island) = body_islands.get_mut(entity) {
            body_island.island_id = new_island_id;
            body_island.prev = None;
            body_island.next = None;
        }
    }

    // --- 3. Re-parent to new world (deferred — only affects hierarchy lookup) ---
    commands.entity(entity).insert(ChildOf(target_world));

    // --- 5. Add collider proxy to new world's tree (direct) ---
    // Collect child collider entities first to avoid borrow conflicts.
    let child_colliders: Vec<Entity> = collider_keys
        .get(entity)
        .ok()
        .and_then(|(_, body_colliders, ..)| body_colliders.map(|c| c.iter().collect()))
        .unwrap_or_default();

    add_proxy_to_world(entity, target_world, &mut collider_keys, &bodies, &mut world_state);
    for collider in child_colliders {
        add_proxy_to_world(collider, target_world, &mut collider_keys, &bodies, &mut world_state);
    }

    // --- 6. Wake the transferred entity and affected islands ---
    // Remove Sleeping and reset sleep timer so physics processes the entity.
    commands.entity(entity).try_remove::<Sleeping>();
    if let Ok(mut timer) = sleep_timers.get_mut(entity) {
        timer.0 = 0.0;
    }

    // Wake the old island so remaining bodies react to the gap.
    // WakeIslands handles setting is_sleeping=false AND removing Sleeping from bodies.
    if old_island_survived {
        commands.queue(WakeIslands {
            world_entity: old_world,
            islands: vec![old_island_id],
        });
    }

    // Wake all sleeping islands in the destination world.
    let new_to_wake: Vec<_> = world_state
        .get(target_world)
        .ok()
        .map(|(_, _, islands, _)| {
            islands.iter().filter(|i| i.is_sleeping).map(|i| i.id).collect()
        })
        .unwrap_or_default();
    if !new_to_wake.is_empty() {
        commands.queue(WakeIslands {
            world_entity: target_world,
            islands: new_to_wake,
        });
    }

    // --- 7. Reframe velocity into the destination frame ---
    // A body keeping pace with the source frame must arrive keeping pace with the
    // destination frame, not hurtling: subtract the destination frame's velocity, add the
    // source frame's. Frame anchors carry these only when they are kinematic movers (the
    // "treadmill" ship); static frames default to zero.
    //
    // NOTE: this is the translational (+ spin-about-anchor) reframe. The ω×r lever-arm
    // between rotating *offset* frames is a refinement for when such frames are used.
    let frame_lin =
        |w: Entity| frame_vels.get(w).ok().and_then(|(l, _)| l).map_or(LinearVelocity::default().0, |l| l.0);
    let frame_ang =
        |w: Entity| frame_vels.get(w).ok().and_then(|(_, a)| a).map_or(AngularVelocity::default().0, |a| a.0);
    if let Ok((mut lin, mut ang)) = body_vels.get_mut(entity) {
        lin.0 += frame_lin(old_world) - frame_lin(target_world);
        ang.0 += frame_ang(old_world) - frame_ang(target_world);
    }

    // --- 8. Notify observers (avian-side + renderer) of the completed handoff ---
    // Both frames explicit so consumers (e.g. solari's rotational rebase of the previous
    // view-projection) never race the world-cache update above.
    commands.trigger(WorldTransferred {
        entity,
        from: old_world,
        to: target_world,
    });
}

/// Removes an entity's collider proxy from a world's [`ColliderTrees`].
#[allow(clippy::type_complexity)]
fn remove_proxy_from_world(
    entity: Entity,
    world_entity: Entity,
    collider_keys: &mut Query<(
        &mut ColliderTreeProxyKey,
        Option<&RigidBodyColliders>,
        Option<&ColliderOf>,
        &ColliderAabb,
        Option<&CollisionLayers>,
        Has<Sensor>,
        Has<CollisionEventsEnabled>,
        Option<&ActiveCollisionHooks>,
    )>,
    world_state: &mut Query<
        (&mut ColliderTrees, &mut MovedProxies, &mut PhysicsIslands, &mut AwakeIslandBitVec),
        With<PhysicsWorld>,
    >,
) {
    let Ok((proxy_key, ..)) = collider_keys.get(entity) else {
        return;
    };
    if *proxy_key == ColliderTreeProxyKey::PLACEHOLDER {
        return;
    }
    let Ok((mut trees, mut moved_proxies, _, _)) = world_state.get_mut(world_entity) else {
        return;
    };
    let tree = trees.tree_for_type_mut(proxy_key.tree_type());
    tree.remove_proxy(proxy_key.id());
    moved_proxies.remove(proxy_key);
}

/// Adds an entity's collider proxy to a world's [`ColliderTrees`].
#[allow(clippy::type_complexity)]
fn add_proxy_to_world(
    entity: Entity,
    world_entity: Entity,
    collider_keys: &mut Query<(
        &mut ColliderTreeProxyKey,
        Option<&RigidBodyColliders>,
        Option<&ColliderOf>,
        &ColliderAabb,
        Option<&CollisionLayers>,
        Has<Sensor>,
        Has<CollisionEventsEnabled>,
        Option<&ActiveCollisionHooks>,
    )>,
    bodies: &Query<(&RigidBody, Has<RigidBodyDisabled>)>,
    world_state: &mut Query<
        (&mut ColliderTrees, &mut MovedProxies, &mut PhysicsIslands, &mut AwakeIslandBitVec),
        With<PhysicsWorld>,
    >,
) {
    let Ok((
        mut proxy_key,
        _,
        collider_of,
        aabb,
        layers,
        is_sensor,
        has_contact_events,
        active_hooks,
    )) = collider_keys.get_mut(entity)
    else {
        return;
    };

    let (tree_type, is_body_disabled) =
        if let Some(Ok((rb, disabled))) = collider_of.map(|c| bodies.get(c.body)) {
            (ColliderTreeType::from_body(Some(*rb)), disabled)
        } else {
            (ColliderTreeType::Standalone, false)
        };

    let proxy = ColliderTreeProxy {
        collider: entity,
        body: collider_of.map(|c| c.body),
        layers: layers.copied().unwrap_or_default(),
        flags: ColliderTreeProxyFlags::new(
            is_sensor,
            is_body_disabled,
            has_contact_events,
            active_hooks.copied().unwrap_or_default(),
        ),
    };

    let Ok((mut trees, mut moved_proxies, _, _)) = world_state.get_mut(world_entity) else {
        return;
    };

    let tree = trees.tree_for_type_mut(tree_type);
    let proxy_id = tree.add_proxy((*aabb).into(), proxy);
    let new_key = ColliderTreeProxyKey::new(proxy_id, tree_type);
    *proxy_key = new_key;
    moved_proxies.insert(new_key);
}

/// Plugin that spawns the [`MainPhysicsWorld`] entity and registers transfer handling.
pub struct PhysicsWorldPlugin;

impl Plugin for PhysicsWorldPlugin {
    fn build(&self, app: &mut App) {
        // Register the transfer observer.
        app.add_observer(on_transfer_to_world);
    }

    // Spawn the main world in `finish`, *after* every plugin's `build` has run — so any
    // `register_required_components::<PhysicsWorld, _>` (e.g. the solari `SolariFrame`
    // binding) is registered before the `PhysicsWorld` archetype first exists. Spawning
    // in `build` creates that archetype too early and a later registration hits
    // `ArchetypeExists`.
    fn finish(&self, app: &mut App) {
        let entity = app.world_mut().spawn(MainPhysicsWorld).id();
        app.insert_resource(MainPhysicsWorldEntity(entity));
    }
}
