//! Optional integration with `bevy_solari`'s native floating origin and reference
//! frames. Enabled by the `solari` feature.
//!
//! The coupling is one-directional and feature-gated: avian's core never names the
//! renderer — only this module, and only with `--features solari`, does. `bevy_solari`
//! never names avian. [`AvianSolariPlugin`] is the single bridge between the two.

use bevy::prelude::*;
use bevy::solari::prelude::{CameraReframe, SolariCamera};

use crate::world::WorldTransferred;

/// Wires avian's multi-world model to `bevy_solari`'s reference frames.
///
/// Every physics world is automatically a solari reference frame: its descendants (the
/// bodies) ride its pose on the GPU transform table, and a moving world's subtree is
/// re-walked by the GPU frontier — no marker needed. This plugin's one job is the camera
/// handoff: an observer that, on a [`WorldTransferred`] **of the camera**, writes a
///   [`CameraReframe`] from the source and destination world anchors' poses, so the
///   renderer re-expresses last frame's view-projection in the new origin basis
///   (motion-vector-continuous handoff) instead of dropping temporal history at the jump.
///
/// Add it alongside `PhysicsPlugins` and `SolariPlugin` when you want the integration;
/// it is never auto-added, so the feature being on doesn't change a build that doesn't ask.
#[derive(Default)]
pub struct AvianSolariPlugin;

impl Plugin for AvianSolariPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(reframe_camera_on_world_transfer);
    }
}

/// On a camera world handoff, rebase the renderer's previous view-projection into the
/// destination frame's basis. `prev_from_current = G_from⁻¹ · G_to` maps a current-basis
/// (destination) world point back into the previous frame's (source) basis, so solari's
/// cached `prev_clip_from_world · prev_from_current` reprojects it for one frame, then
/// re-caches in the new basis.
///
/// Only fires when the transferred entity is itself a [`SolariCamera`] — a body changing
/// worlds doesn't move the view basis. The world anchors carry the kinematic state, so
/// reading their `GlobalTransform` here never races the transfer's world-cache update
/// ([`WorldTransferred`] is emitted *after* migration completes).
fn reframe_camera_on_world_transfer(
    trigger: On<WorldTransferred, ()>,
    anchors: Query<&GlobalTransform>,
    mut cameras: Query<&mut CameraReframe, With<SolariCamera>>,
) {
    let entity = trigger.event_target();
    let event = trigger.event();
    let Ok(mut reframe) = cameras.get_mut(entity) else {
        return;
    };
    let (Ok(g_from), Ok(g_to)) = (anchors.get(event.from), anchors.get(event.to)) else {
        return;
    };
    // Affine3A inverse·compose, downcast to the Mat4 solari post-multiplies the cached
    // prev_clip_from_world by. If a single-handoff test shows geometry double-rotating,
    // the basis convention is inverted — flip to `G_to⁻¹ · G_from`.
    let prev_from_current = g_from.affine().inverse() * g_to.affine();
    *reframe = CameraReframe::from_prev_from_current(Mat4::from(prev_from_current));
}
