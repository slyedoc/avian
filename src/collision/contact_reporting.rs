//! Sends collision events and updates [`CollidingEntities`].
//!
//! See [`ContactReportingPlugin`].

use crate::prelude::*;
use bevy::prelude::*;

/// Sends collision events and updates [`CollidingEntities`].
///
/// # Collision Events
///
/// If the [`ContactReportingPlugin`] is enabled (the default), the following
/// collision events are sent each frame in [`PhysicsStepSet::ReportContacts`]:
///
/// - [`Collision`]
/// - [`CollisionStarted`]
/// - [`CollisionEnded`]
///
/// You can listen to them with normal event readers:
///
/// ```no_run
#[cfg_attr(feature = "2d", doc = "use avian2d::prelude::*;")]
#[cfg_attr(feature = "3d", doc = "use avian3d::prelude::*;")]
/// use bevy::prelude::*;
///
/// fn main() {
///     App::new()
///         .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
///         .add_systems(Update, print_collisions)
///         .run();
/// }
///
/// fn print_collisions(mut collision_event_reader: EventReader<Collision>) {
///     for Collision(contacts) in collision_event_reader.read() {
///         println!(
///             "Entities {} and {} are colliding",
///             contacts.entity1,
///             contacts.entity2,
///         );
///     }
/// }
/// ```
pub struct ContactReportingPlugin;

impl Plugin for ContactReportingPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Collision>()
            .add_event::<CollisionStarted>()
            .add_event::<CollisionEnded>();

        let physics_schedule = app
            .get_schedule_mut(PhysicsSchedule)
            .expect("add PhysicsSchedule first");

        physics_schedule.add_systems(report_contacts.in_set(PhysicsStepSet::ReportContacts));
    }

    fn finish(&self, app: &mut App) {
        // Register timer and counter diagnostics for collision detection.
        app.register_physics_diagnostics::<CollisionDiagnostics>();
    }
}

/// A [collision event](ContactReportingPlugin#collision-events)
/// that is sent for each collision.
///
/// # Example
///
/// ```no_run
#[cfg_attr(feature = "2d", doc = "use avian2d::prelude::*;")]
#[cfg_attr(feature = "3d", doc = "use avian3d::prelude::*;")]
/// use bevy::prelude::*;
///
/// fn main() {
///     App::new()
///         .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
///         .add_systems(Update, print_collisions)
///         .run();
/// }
///
/// fn print_collisions(mut collision_event_reader: EventReader<Collision>) {
///     for Collision(contacts) in collision_event_reader.read() {
///         println!(
///             "Entities {} and {} are colliding",
///             contacts.entity1,
///             contacts.entity2,
///         );
///     }
/// }
/// ```
#[derive(Event, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Collision(pub Contacts);

/// A [collision event](ContactReportingPlugin#collision-events)
/// that is sent when two entities start colliding.
///
/// # Example
///
/// ```no_run
#[cfg_attr(feature = "2d", doc = "use avian2d::prelude::*;")]
#[cfg_attr(feature = "3d", doc = "use avian3d::prelude::*;")]
/// use bevy::prelude::*;
///
/// fn main() {
///     App::new()
///         .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
///         .add_systems(Update, print_started_collisions)
///         .run();
/// }
///
/// fn print_started_collisions(mut collision_event_reader: EventReader<CollisionStarted>) {
///     for CollisionStarted(entity1, entity2) in collision_event_reader.read() {
///         println!(
///             "Entities {} and {} started colliding",
///             entity1,
///             entity2,
///         );
///     }
/// }
/// ```
#[derive(Event, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct CollisionStarted(pub Entity, pub Entity);

/// A [collision event](ContactReportingPlugin#collision-events)
/// that is sent when two entities stop colliding.
///
/// # Example
///
/// ```no_run
#[cfg_attr(feature = "2d", doc = "use avian2d::prelude::*;")]
#[cfg_attr(feature = "3d", doc = "use avian3d::prelude::*;")]
/// use bevy::prelude::*;
///
/// fn main() {
///     App::new()
///         .add_plugins((DefaultPlugins, PhysicsPlugins::default()))
///         .add_systems(Update, print_ended_collisions)
///         .run();
/// }
///
/// fn print_ended_collisions(mut collision_event_reader: EventReader<CollisionEnded>) {
///     for CollisionEnded(entity1, entity2) in collision_event_reader.read() {
///         println!(
///             "Entities {} and {} stopped colliding",
///             entity1,
///             entity2,
///         );
///     }
/// }
/// ```
#[derive(Event, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct CollisionEnded(pub Entity, pub Entity);

/// Sends collision events and updates [`CollidingEntities`].
pub fn report_contacts(
    mut colliders: Query<&mut CollidingEntities>,
    collisions: Res<Collisions>,
    mut collision_ev_writer: EventWriter<Collision>,
    mut collision_started_ev_writer: EventWriter<CollisionStarted>,
    mut collision_ended_ev_writer: EventWriter<CollisionEnded>,
    mut diagnostics: ResMut<CollisionDiagnostics>,
) {
    let start = crate::utils::Instant::now();

    // TODO: Would batching events be worth it?
    for ((entity1, entity2), contacts) in collisions.get_internal().iter() {
        if contacts.during_current_frame {
            collision_ev_writer.write(Collision(contacts.clone()));

            // Collision started
            if !contacts.during_previous_frame {
                collision_started_ev_writer.write(CollisionStarted(*entity1, *entity2));

                if let Ok(mut colliding_entities1) = colliders.get_mut(*entity1) {
                    colliding_entities1.insert(*entity2);
                }
                if let Ok(mut colliding_entities2) = colliders.get_mut(*entity2) {
                    colliding_entities2.insert(*entity1);
                }
            }
        }

        // Collision ended
        if !contacts.during_current_frame && contacts.during_previous_frame {
            collision_ended_ev_writer.write(CollisionEnded(*entity1, *entity2));

            if let Ok(mut colliding_entities1) = colliders.get_mut(*entity1) {
                colliding_entities1.remove(entity2);
            }
            if let Ok(mut colliding_entities2) = colliders.get_mut(*entity2) {
                colliding_entities2.remove(entity1);
            }
        }
    }

    diagnostics.collision_events = start.elapsed();
}
