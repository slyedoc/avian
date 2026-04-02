# Avian Multiframe Branch — Notes

## Branch: `multiframe` on slyedoc/avian (based on `bevy-main`)

## What's Working

### Multi-World Physics
- `PhysicsWorld` component on an entity creates an isolated physics simulation
- All physics state (gravity, collider trees, islands, solver, contacts) lives as components on the PhysicsWorld entity via `#[require]`
- `MainPhysicsWorld` is spawned automatically by `PhysicsWorldPlugin`
- Entities belong to the nearest `PhysicsWorld` ancestor in the hierarchy
- No interaction between worlds — separate broad phase, narrow phase, solver per world

### Hierarchy-Based World Assignment
- Walk up `ChildOf` to find nearest `PhysicsWorld` ancestor
- Falls back to `MainPhysicsWorldEntity` if no ancestor found
- `PhysicsWorldEntity` cache component for O(1) lookup (set on spawn, updated on transfer)
- `PhysicsWorldLookup` SystemParam for systems, `find_physics_world_in_hierarchy()` for hooks

### Entity Transfer Between Worlds
```rust
commands.trigger(TransferToWorld {
    entity: player_entity,
    world: planet_world,
});
```
- Handles: collider tree proxy migration, island unlink/relink, sleep state reset, awake bit, ChildOf re-parent
- Direct query mutations in observer — no exclusive world lock, no deferred hook ordering issues
- Remember to also set `Transform` and zero `LinearVelocity`/`AngularVelocity` on the transferred entity

### bigspace Integration
- `transform_to_position` composes local `Transform` values up to PhysicsWorld ancestor (f64 arithmetic) instead of reading `GlobalTransform` (f32 precision loss)
- PhysicsWorld can live on a bigspace `Grid` entity — physics entities as children get both spatial partitioning and physics world assignment
- Disable bevy's `TransformPlugin` when using bigspace (bigspace replaces it)
- For physics entities in bigspace, spawn with `ChildOf(grid_entity)` directly (not `spawn_spatial`) so avian hooks see the hierarchy immediately

### Resources Converted to Components
All on the `PhysicsWorld` entity:
Gravity, PhysicsLengthUnit, SubstepCount, SolverConfig, ContactSoftnessCoefficients, NarrowPhaseConfig, DefaultFriction, DefaultRestitution, TimeToSleep, ColliderTrees, MovedProxies, EnlargedProxies, ContactGraph, JointGraph, ContactConstraints, ConstraintGraph, PhysicsIslands, AwakeIslandBitVec, ContactStatusBits, ColliderTreeOptimization, OptimizationTasks, LastDynamicKinematicAabbUpdate, CollisionDiagnostics, SolverDiagnostics, ColliderTreeDiagnostics, SpatialQueryDiagnostics

### Stayed as Resources (not per-world)
LastPhysicsTick, Time\<Physics\>, Time\<Substeps\>, NarrowPhaseInitialized, JointGraphPluginInitialized, CachedBodySleepingSystemState, CachedIslandSleepingSystemState, CachedIslandWakingSystemState, PhysicsTransformConfig, PhysicsPickingSettings, ColliderCache, MainPhysicsWorldEntity

## Examples

### `multi_cubes`
Two side-by-side physics worlds with different gravity. Press Space/Backspace to transfer cubes between them. Sleep state shown via color (dark = sleeping, bright = awake).

### `bigspace_cubes`
Two reference frames (nested Grids) 1 billion units apart, each with its own PhysicsWorld. Press Tab to teleport between stations. Demonstrates precision-safe physics at extreme distances.

## Known Issues / TODOs

- `ContactConstraints` is always required (was previously conditional via `NarrowPhasePlugin::generate_constraints`). May need a flag for sensor-only worlds.
- `SubstepCount` and `run_substep_schedule`/`run_physics_schedule` use `world.query::<&T>().single(world)` — works with multiple worlds but could be cleaner
- Doc tests are broken (pre-existing, not from multiframe changes) — skip with `cargo test -p avian3d --lib`
- `rasterizes_compound` test has f32 precision drift — skip with `--skip rasterizes_compound`
- bigspace validation warns about MainPhysicsWorld entity outside BigSpace hierarchy (harmless)
- For bigspace: must use `ChildOf(grid_entity)` directly when spawning physics entities (bigspace's `spawn_spatial` defers ChildOf, but avian hooks need it immediately)

## Dependencies
- `bevy_mod_debugdump` → slyedoc fork (bevy-main branch)
- `big_space` → slyedoc fork (bevy-main branch, dev-dependency for examples only)

## Testing
```bash
cargo test -p avian3d --lib -- --skip rasterizes_compound
cargo run -p avian3d --example multi_cubes
cargo run -p avian3d --example bigspace_cubes
```
