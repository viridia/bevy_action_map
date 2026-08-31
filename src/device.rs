//! Devices: enumeration, identity, capabilities, and calibration.
//!
//! This module models devices with a runtime handle, a persistent identity, and capability data
//! used by prompts, pairing, and calibration.

/// A device as it exists right now, in this running process.
///
/// Not persistent: a gamepad's [`Entity`](bevy_ecs::entity::Entity) is reassigned by the backend
/// on every reconnect, so nothing should compare a saved `DeviceHandle` against a live one across a
/// restart. Surviving a reconnect needs a stable identity, which is a separate, not-yet-built
/// mechanism (R11.5).
///
/// The keyboard and mouse are modeled as one device, `KeyboardMouse` — this crate has never treated
/// them as separable, and [`RawEvent::MouseMotion`](crate::frame::RawEvent::MouseMotion) is
/// unconditionally part of the frame even with both features off, so this variant is unconditional
/// too, for the same reason: an enum a `match` can be written against without a feature-gated
/// catch-all arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceHandle {
    /// The keyboard and mouse, treated as one device.
    KeyboardMouse,
    /// One connected gamepad, identified by the backend's own entity for it.
    #[cfg(feature = "gamepad")]
    Gamepad(bevy_ecs::entity::Entity),
}

impl DeviceHandle {
    /// Which binding scheme this device's controls belong to.
    pub const fn scheme(self) -> crate::mapping::Scheme {
        match self {
            Self::KeyboardMouse => crate::mapping::Scheme::KeyboardMouse,
            #[cfg(feature = "gamepad")]
            Self::Gamepad(_) => crate::mapping::Scheme::Gamepad,
        }
    }
}

/// The devices one occupant has claimed.
///
/// A plain value type, not a component — [`Paired`](crate::player::Paired) is the component that
/// attaches one of these to a context entity, and keeping the set itself component-free is what
/// lets it be read outside a query: constructed inline in a test, or passed to a future
/// presentation filter without threading `Option<&Paired>` through a query tuple.
///
/// Backed by a small inline array rather than a hard cap: a handful of devices per occupant is the
/// common case (a keyboard and mouse plus a pad or two), and a fifth device spills to the heap
/// instead of being silently dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceHandleSet(smallvec::SmallVec<[DeviceHandle; 4]>);

impl DeviceHandleSet {
    /// A set claiming exactly one device.
    pub fn of(device: DeviceHandle) -> Self {
        let mut devices = smallvec::SmallVec::new();
        devices.push(device);
        Self(devices)
    }

    /// Adds a device to the set.
    pub fn insert(&mut self, device: DeviceHandle) {
        if !self.contains(device) {
            self.0.push(device);
        }
    }

    /// Whether this set claims the given device.
    pub fn contains(&self, device: DeviceHandle) -> bool {
        self.0.contains(&device)
    }

    /// The claimed devices, in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = DeviceHandle> + '_ {
        self.0.iter().copied()
    }

    /// The claimed device belonging to the given scheme, if any.
    ///
    /// An occupant with one device per scheme has at most one answer; a game that pairs two devices
    /// of the same scheme to one occupant gets whichever was claimed first.
    pub fn owner_for(&self, scheme: crate::mapping::Scheme) -> Option<DeviceHandle> {
        self.0
            .iter()
            .copied()
            .find(|device| device.scheme() == scheme)
    }
}

impl FromIterator<DeviceHandle> for DeviceHandleSet {
    fn from_iter<T: IntoIterator<Item = DeviceHandle>>(iter: T) -> Self {
        let mut set = Self::default();
        for device in iter {
            set.insert(device);
        }
        set
    }
}

#[cfg(feature = "gamepad")]
use bevy_ecs::entity::Entity;
#[cfg(feature = "gamepad")]
use bevy_ecs::prelude::{Changed, Query, Resource};
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{AxisSettings, GamepadAxis, GamepadSettings};
#[cfg(feature = "gamepad")]
use bevy_platform::collections::HashMap;

/// Where one gamepad axis rests, and how far it wanders there.
///
/// This is the *hardware* correction, applied to every raw reading before any binding sees it. It
/// removes drift — a stick that no longer returns to zero, and the jitter around wherever it does
/// return to — and nothing else. What a mechanic wants from a stick is the binding's own
/// [`DeadZone`](crate::binding::DeadZone), which runs after this, and which is the stage that
/// rescales.
///
/// Two stages rather than one number because they answer different questions and only one of them
/// is knowable in advance: a game can say what its turning mechanic needs, but not how worn this
/// particular player's left stick is.
///
/// The default is the identity — centred at zero, wandering not at all — which is what an
/// uncalibrated pad is taken to do.
#[cfg(feature = "gamepad")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisCalibration {
    /// What this axis reads when nothing is touching it.
    pub center: f32,
    /// How far either side of [`center`](Self::center) still counts as untouched.
    pub rest: f32,
}

#[cfg(feature = "gamepad")]
impl AxisCalibration {
    /// Corrects one raw reading: recentre, then suppress anything inside the rest envelope.
    ///
    /// Deliberately does not rescale. At most one stage may — otherwise a threshold stops denoting
    /// any particular stick position — and that one is the binding's, which is the only threshold a
    /// player was ever shown a number for.
    pub fn apply(self, raw: f32) -> f32 {
        let centered = raw - self.center;
        if centered.abs() <= self.rest {
            return 0.0;
        }
        // Recentring can push a fully deflected axis past 1.0, which would then survive a
        // binding's own rescale and hand an action a value out of range.
        centered.clamp(-1.0, 1.0)
    }
}

/// What each connected gamepad's axes do when nobody is touching them.
///
/// Empty by default, which reads as "every axis is honest" — a game that never touches this gets
/// exactly the behavior it had before. Fill it from [`CalibrationSampling`], or
/// [`set`](Self::set) a value directly for a game that lets the player enter one.
///
/// Keyed by the backend's entity for the pad, so nothing here survives a reconnect. Saving
/// calibration across a restart needs a device identity that is stable across one, which this crate
/// does not have yet.
#[cfg(feature = "gamepad")]
#[derive(Resource, Default, Debug)]
pub struct GamepadCalibration {
    axes: HashMap<(Entity, GamepadAxis), AxisCalibration>,
}

#[cfg(feature = "gamepad")]
impl GamepadCalibration {
    /// The calibration for one axis, or the identity if it has never been calibrated.
    pub fn get(&self, gamepad: Entity, axis: GamepadAxis) -> AxisCalibration {
        self.axes.get(&(gamepad, axis)).copied().unwrap_or_default()
    }

    /// Sets the calibration for one axis.
    pub fn set(&mut self, gamepad: Entity, axis: GamepadAxis, calibration: AxisCalibration) {
        self.axes.insert((gamepad, axis), calibration);
    }

    /// Forgets everything measured about one device.
    pub fn clear_device(&mut self, gamepad: Entity) {
        self.axes.retain(|(entity, _), _| *entity != gamepad);
    }

    /// Whether anything has been calibrated at all.
    pub fn is_empty(&self) -> bool {
        self.axes.is_empty()
    }

    /// Corrects one raw reading, using this axis's calibration if it has one.
    pub fn apply(&self, gamepad: Entity, axis: GamepadAxis, raw: f32) -> f32 {
        self.get(gamepad, axis).apply(raw)
    }
}

/// How much wider than the observed spread a measured rest envelope is made.
///
/// A sampling step sees a few seconds of a stick that will be resting for hours, so the widest
/// wander it happened to catch is a floor rather than the answer.
#[cfg(feature = "gamepad")]
const REST_MARGIN: f32 = 1.25;

/// What one axis was seen doing during a calibration step.
#[cfg(feature = "gamepad")]
#[derive(Clone, Copy, Debug)]
struct RestSample {
    min: f32,
    max: f32,
}

/// Collects what the sticks do while nobody is touching them.
///
/// Insert it as a resource to begin an explicit "let go of the sticks" step, and remove it to end
/// one. While it is present, every raw axis reading is offered to it; when enough has arrived, hand
/// what it saw to a [`GamepadCalibration`] with [`finish`](Self::finish).
///
/// Driven by the game rather than running in the background, deliberately: a stick that happens to
/// be deflected while a background detector is learning would be learned as centre, and there is no
/// way for the detector to know it should not be.
///
/// **Ask the player to move the sticks and let go**, rather than only to hold still. A gamepad
/// reports an axis when it *changes*, so a stick that settled before the step began reports nothing
/// during it and is left uncalibrated — which is the case that most needs calibrating, since a
/// stick resting steadily off centre is exactly what drift looks like. Releasing one during the
/// step guarantees a reading at whatever it now rests at.
#[cfg(feature = "gamepad")]
#[derive(Resource, Default, Debug)]
pub struct CalibrationSampling {
    seen: HashMap<(Entity, GamepadAxis), RestSample>,
}

#[cfg(feature = "gamepad")]
impl CalibrationSampling {
    /// Offers one raw reading to the sample.
    ///
    /// Called for you while this resource exists. It takes the reading *before* correction, so that
    /// running a second calibration step does not measure the first one's output.
    pub fn observe(&mut self, gamepad: Entity, axis: GamepadAxis, raw: f32) {
        self.seen
            .entry((gamepad, axis))
            .and_modify(|sample| {
                sample.min = sample.min.min(raw);
                sample.max = sample.max.max(raw);
            })
            .or_insert(RestSample { min: raw, max: raw });
    }

    /// How many axes have reported anything so far.
    ///
    /// A step that ends at zero measured nothing, which a screen may want to say rather than
    /// silently claiming success.
    pub fn axes_seen(&self) -> usize {
        self.seen.len()
    }

    /// Writes what was seen into a calibration set.
    ///
    /// Centre is the midpoint of the readings and the envelope is half their spread, widened. An
    /// axis that reported nothing is left alone rather than reset, so one pad going quiet during
    /// the step does not discard what another step already learned about it.
    pub fn finish(&self, into: &mut GamepadCalibration) {
        for (&(gamepad, axis), sample) in self.seen.iter() {
            into.set(
                gamepad,
                axis,
                AxisCalibration {
                    center: (sample.min + sample.max) / 2.0,
                    rest: (sample.max - sample.min) / 2.0 * REST_MARGIN,
                },
            );
        }
    }
}

/// Warns about gamepad settings this crate does not honour.
///
/// Bevy's own `GamepadSettings` deadzones and thresholds are applied when it converts a raw gamepad
/// message into a processed one. This crate reads the raw message instead — a clamp applied below
/// you cannot be undone above you, and owning the whole chain is the only way a game can ask for a
/// deadzone smaller than the one someone underneath already applied. The cost is that a game which
/// configures `GamepadSettings` and expects it to reach a binding gets silence, so this says so
/// once.
#[cfg(feature = "gamepad")]
pub fn warn_on_unread_gamepad_settings(
    settings: Query<&GamepadSettings, Changed<GamepadSettings>>,
) {
    if settings.iter().any(is_customized) {
        bevy_utils::once!(log::warn!(
            "a gamepad's `GamepadSettings` has been customized, but `bevy_action_map` reads raw \
             gamepad messages, which Bevy emits before those settings are applied — so they reach \
             no binding. Put the deadzone on the binding with `dead_zone`, or correct the hardware \
             with `GamepadCalibration`."
        ));
    }
}

/// Whether a gamepad's settings have been moved off Bevy's own defaults.
// `AxisSettings` is the only one of the three with `PartialEq`, so the per-control maps are tested
// for being populated at all rather than compared field by field against a default.
#[cfg(feature = "gamepad")]
fn is_customized(settings: &GamepadSettings) -> bool {
    settings.default_axis_settings != AxisSettings::default()
        || !settings.axis_settings.is_empty()
        || !settings.button_settings.is_empty()
        || !settings.button_axis_settings.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_deduplicates_and_reports_containment() {
        let mut set = DeviceHandleSet::of(DeviceHandle::KeyboardMouse);
        set.insert(DeviceHandle::KeyboardMouse);
        assert_eq!(set.iter().count(), 1);
        assert!(set.contains(DeviceHandle::KeyboardMouse));
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn calibration_recentres_and_suppresses_the_rest_envelope() {
        // A stick that rests at +0.1 and wanders 0.02 either side of it.
        let drifting = AxisCalibration {
            center: 0.1,
            rest: 0.02,
        };

        // Everything inside the envelope is the stick doing nothing.
        assert_eq!(drifting.apply(0.1), 0.0);
        assert_eq!(drifting.apply(0.115), 0.0);
        assert_eq!(drifting.apply(0.085), 0.0);
        // And just outside it is not. The boundary itself is left untested on purpose: it lands
        // where float subtraction says it does, and nothing should be built on which side.
        assert!(drifting.apply(0.15) > 0.0);

        // Outside it, the reading is corrected but not stretched: a deadzone that rescaled here
        // would leave the binding's own threshold denoting no particular stick position.
        assert!((drifting.apply(0.5) - 0.4).abs() < 1e-6);
        assert!((drifting.apply(-0.5) - -0.6).abs() < 1e-6);

        // Recentring pushes full deflection past the end of the range, which is clamped rather
        // than passed on — a binding's own rescale would otherwise hand an action more than 1.0.
        assert_eq!(drifting.apply(-1.0), -1.0);

        // An uncalibrated axis is left exactly alone, which is what an empty set has to mean.
        assert_eq!(AxisCalibration::default().apply(0.03), 0.03);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn sampling_measures_a_centre_and_an_envelope() {
        let pad = bevy_ecs::entity::Entity::from_bits(1);
        let mut sampling = CalibrationSampling::default();
        for value in [0.08, 0.12, 0.10, 0.09, 0.11] {
            sampling.observe(pad, GamepadAxis::LeftStickX, value);
        }
        assert_eq!(sampling.axes_seen(), 1);

        let mut calibration = GamepadCalibration::default();
        sampling.finish(&mut calibration);

        let measured = calibration.get(pad, GamepadAxis::LeftStickX);
        assert!((measured.center - 0.10).abs() < 1e-6);
        // Half the observed spread, widened: a few seconds of samples is a floor on what a stick
        // resting for hours will do.
        assert!((measured.rest - 0.02 * REST_MARGIN).abs() < 1e-6);
        // And the whole point of measuring: the rest position now reads as untouched.
        assert_eq!(measured.apply(0.10), 0.0);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn calibration_is_per_unit_and_forgettable() {
        let worn = bevy_ecs::entity::Entity::from_bits(1);
        let fresh = bevy_ecs::entity::Entity::from_bits(2);
        let mut calibration = GamepadCalibration::default();
        calibration.set(
            worn,
            GamepadAxis::LeftStickX,
            AxisCalibration {
                center: 0.1,
                rest: 0.05,
            },
        );

        // Drift is a characteristic of the individual unit, so a second pad of the same model is
        // not corrected by what the first one needed.
        assert_eq!(calibration.apply(worn, GamepadAxis::LeftStickX, 0.1), 0.0);
        assert_eq!(calibration.apply(fresh, GamepadAxis::LeftStickX, 0.1), 0.1);
        // Nor is a different axis on the same pad.
        assert_eq!(calibration.apply(worn, GamepadAxis::LeftStickY, 0.1), 0.1);

        calibration.clear_device(worn);
        assert!(calibration.is_empty());
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn a_quiet_axis_keeps_the_calibration_it_had() {
        let pad = bevy_ecs::entity::Entity::from_bits(1);
        let mut calibration = GamepadCalibration::default();
        calibration.set(
            pad,
            GamepadAxis::LeftStickX,
            AxisCalibration {
                center: 0.1,
                rest: 0.05,
            },
        );

        // A pad reports an axis only when it changes, so a step during which one stick never moves
        // measures nothing about it. That must not read as "measured zero drift".
        let mut sampling = CalibrationSampling::default();
        sampling.observe(pad, GamepadAxis::LeftStickY, 0.0);
        sampling.finish(&mut calibration);

        assert_eq!(
            calibration.get(pad, GamepadAxis::LeftStickX).center,
            0.1,
            "an axis that reported nothing was reset rather than left alone"
        );
    }

    /// A pad arrives carrying default settings, so the warning has to be about a game that moved
    /// them — firing on the default would mean warning every game that ever connects a gamepad.
    #[cfg(feature = "gamepad")]
    #[test]
    fn only_settings_moved_off_the_default_are_worth_warning_about() {
        assert!(!is_customized(&GamepadSettings::default()));

        let mut deadzoned = GamepadSettings::default();
        deadzoned.default_axis_settings.set_deadzone_upperbound(0.2);
        assert!(is_customized(&deadzoned));

        let mut per_axis = GamepadSettings::default();
        per_axis
            .axis_settings
            .insert(GamepadAxis::LeftStickX, AxisSettings::default());
        assert!(is_customized(&per_axis));
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn owner_for_finds_the_device_in_its_own_scheme() {
        let pad = bevy_ecs::entity::Entity::from_bits(1);
        let set =
            DeviceHandleSet::from_iter([DeviceHandle::KeyboardMouse, DeviceHandle::Gamepad(pad)]);
        assert_eq!(
            set.owner_for(crate::mapping::Scheme::Gamepad),
            Some(DeviceHandle::Gamepad(pad))
        );
        assert_eq!(
            set.owner_for(crate::mapping::Scheme::KeyboardMouse),
            Some(DeviceHandle::KeyboardMouse)
        );
    }
}
