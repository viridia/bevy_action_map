//! Devices: families, enumeration, identity, brand, and calibration.
//!
//! This module models devices at two grains: a [`DeviceFamily`] is the class of hardware a binding
//! is written for, and a [`DeviceHandle`] is one unit of it plugged in right now. Alongside those
//! are a persistent identity that survives a reconnect, the brand a prompt names a pad's buttons
//! from, and per-device calibration.

// Named so the `#[reflect(..)]` attributes on `DeviceId` resolve; not referred to directly.
#[cfg(feature = "serialize")]
use bevy_reflect::serde::{ReflectDeserializeWithRegistry, ReflectSerializeWithRegistry};

/// The set of devices a player is using, and the scope a rebinding is made in.
///
/// Keyboard bindings and gamepad bindings are alternatives rather than competitors: a player is
/// using one or the other at any moment, so the two never conflict with each other and are remapped
/// independently. A rebinding screen shows one family at a time for the same reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DeviceFamily {
    /// Keyboard and mouse.
    KeyboardMouse,
    /// A gamepad.
    Gamepad,
}

/// A device as it exists right now, in this running process.
///
/// Not persistent: a gamepad's `Entity` is reassigned by the backend on every reconnect, so nothing
/// should compare a saved `DeviceHandle` against a live one across a restart. Recognizing a device
/// that comes back is [`DeviceId`]'s job, carried on the device's own entity as [`Identity`].
///
/// The keyboard and mouse are modeled as one device, `KeyboardMouse`, since this crate has never
/// treated them as separable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceHandle {
    // Unconditional even with both device features off: `RawEvent::MouseMotion` is always part of
    // the frame, and this keeps a `match` on `DeviceHandle` exhaustive without a feature-gated
    // catch-all arm.
    /// The keyboard and mouse, treated as one device.
    KeyboardMouse,
    /// One connected gamepad, identified by the backend's own entity for it.
    #[cfg(feature = "gamepad")]
    Gamepad(bevy_ecs::entity::Entity),
}

impl DeviceHandle {
    /// Which binding family this device's controls belong to.
    pub const fn family(self) -> DeviceFamily {
        match self {
            Self::KeyboardMouse => DeviceFamily::KeyboardMouse,
            #[cfg(feature = "gamepad")]
            Self::Gamepad(_) => DeviceFamily::Gamepad,
        }
    }
}

/// The devices one occupant has claimed.
///
/// A plain value type, not itself a component — attach it to a context entity with
/// [`Paired`](crate::player::Paired), which is what makes an occupant's claimed devices queryable.
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

    /// The claimed device belonging to the given family, if any.
    ///
    /// An occupant with one device per family has at most one answer; a game that pairs two devices
    /// of the same family to one occupant gets whichever was claimed first.
    pub fn owner_for(&self, family: DeviceFamily) -> Option<DeviceHandle> {
        self.0
            .iter()
            .copied()
            .find(|device| device.family() == family)
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

/// A device identity that outlives the process, defined by whatever backend knows how to produce
/// one.
///
/// A [`DeviceHandle`] names a device plugged in right now. This names the device *itself*, so a
/// pairing or a calibration can be stored against it and found again after a restart. What counts
/// as identity is the backend's business — a USB vendor and product id, a platform handle, a serial
/// number — so the payload is your own type and this crate never looks inside it.
///
/// ```
/// # use bevy_action_map::device::DeviceIdentity;
/// # use bevy_reflect::Reflect;
/// #[derive(Reflect, Clone, Debug, PartialEq, Eq, Hash)]
/// struct UsbDeviceId {
///     vendor: u16,
///     product: u16,
/// }
///
/// impl DeviceIdentity for UsbDeviceId {
///     const DOMAIN: &'static str = "usb";
/// }
/// ```
///
/// `DOMAIN` names your identity in a save file, and it is a name you choose rather than the Rust
/// path of the type: renaming the type or moving it between modules must not orphan a player's
/// saved pairings. Keep it short, specific to the backend, and fixed once you have shipped it.
///
/// `Eq` and `Hash` are required because saved settings are stored keyed by identity, so an identity
/// holding a float cannot be used here. Take them from `derive`; an identity that is expensive to
/// compare is an identity that is too big.
///
/// # Surviving a change to your own type
///
/// By default the payload is written field by field, which means a release that adds or removes a
/// field can no longer read what the last one wrote. If you expect the type to change, give it its
/// own `Serialize` and `Deserialize` and add `#[reflect(Serialize, Deserialize)]`: you then decide
/// what the stored form looks like and what it tolerates, and a compact string is usually the
/// easiest thing to keep reading. It is also the only way to store a value above `i64::MAX`, which
/// some settings formats cannot hold as a number.
#[cfg(feature = "bevy_reflect")]
pub trait DeviceIdentity:
    bevy_reflect::Reflect
    + bevy_reflect::FromReflect
    + bevy_reflect::GetTypeRegistration
    + Clone
    + Eq
    + core::hash::Hash
    + core::fmt::Debug
{
    /// The name this identity is stored under. Yours to choose, and stable once shipped.
    const DOMAIN: &'static str;
}

/// What [`DeviceId`] needs from a payload whose type it has forgotten, captured while the type is
/// still known.
///
/// Taken from the trait bounds rather than from
/// `reflect_clone`/`reflect_partial_eq`/`reflect_hash`: the derive special-cases those three, so a
/// backend that omits `#[reflect(Hash)]` cannot be detected at registration and would instead panic
/// the first time its device was plugged in. See TD7.7.
#[cfg(feature = "bevy_reflect")]
#[derive(Clone, Copy)]
struct DeviceIdOps {
    domain: &'static str,
    hash: fn(&dyn bevy_reflect::Reflect, &mut dyn core::hash::Hasher),
    eq: fn(&dyn bevy_reflect::Reflect, &dyn bevy_reflect::Reflect) -> bool,
    clone: fn(&dyn bevy_reflect::Reflect) -> alloc::boxed::Box<dyn bevy_reflect::Reflect>,
    debug: fn(&dyn bevy_reflect::Reflect, &mut core::fmt::Formatter<'_>) -> core::fmt::Result,
}

#[cfg(feature = "bevy_reflect")]
impl DeviceIdOps {
    fn of<T: DeviceIdentity>() -> Self {
        // Every one of these is handed the payload it was built beside: `DeviceId::new` and the
        // deserializer are the only sites that pair the two, and both are generic over this `T`.
        fn cast<T: DeviceIdentity>(value: &dyn bevy_reflect::Reflect) -> &T {
            debug_assert!(
                value.downcast_ref::<T>().is_some(),
                "`DeviceIdOps` was paired with a payload of another type"
            );
            value.downcast_ref::<T>().unwrap()
        }

        Self {
            domain: T::DOMAIN,
            // `&mut &mut dyn Hasher` is the sized receiver `Hash::hash` wants.
            hash: |value, mut state| core::hash::Hash::hash(cast::<T>(value), &mut state),
            eq: |left, right| {
                right
                    .downcast_ref::<T>()
                    .is_some_and(|right| cast::<T>(left) == right)
            },
            clone: |value| alloc::boxed::Box::new(cast::<T>(value).clone()),
            debug: |value, f| core::fmt::Debug::fmt(cast::<T>(value), f),
        }
    }
}

/// One device's persistent identity, whichever backend produced it.
///
/// Compare them, hash them, store them; this crate never inspects what is inside one. Build one
/// from your own [`DeviceIdentity`] type with [`new`](Self::new), and ask [`domain`](Self::domain)
/// which backend an identity came from.
///
/// Two identities from different backends are never equal, even if their payloads happen to hold
/// the same bytes.
#[cfg(feature = "bevy_reflect")]
#[derive(bevy_reflect::Reflect)]
#[reflect(opaque)]
#[reflect(PartialEq, Hash, Debug)]
#[cfg_attr(
    feature = "serialize",
    reflect(SerializeWithRegistry, DeserializeWithRegistry)
)]
pub struct DeviceId {
    payload: alloc::boxed::Box<dyn bevy_reflect::Reflect>,
    ops: DeviceIdOps,
}

#[cfg(feature = "bevy_reflect")]
impl DeviceId {
    /// Wraps a backend's own identity type.
    pub fn new<T: DeviceIdentity>(identity: T) -> Self {
        Self {
            payload: alloc::boxed::Box::new(identity),
            ops: DeviceIdOps::of::<T>(),
        }
    }

    /// Which backend this identity belongs to — the [`DOMAIN`](DeviceIdentity::DOMAIN) of the type
    /// it was built from.
    pub fn domain(&self) -> &'static str {
        self.ops.domain
    }

    /// The identity as the backend's own type, or `None` if it came from a different one.
    pub fn get<T: DeviceIdentity>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }
}

#[cfg(feature = "bevy_reflect")]
impl Clone for DeviceId {
    fn clone(&self) -> Self {
        Self {
            payload: (self.ops.clone)(&*self.payload),
            ops: self.ops,
        }
    }
}

#[cfg(feature = "bevy_reflect")]
impl PartialEq for DeviceId {
    fn eq(&self, other: &Self) -> bool {
        self.ops.domain == other.ops.domain && (self.ops.eq)(&*self.payload, &*other.payload)
    }
}

#[cfg(feature = "bevy_reflect")]
impl Eq for DeviceId {}

#[cfg(feature = "bevy_reflect")]
impl core::hash::Hash for DeviceId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // The domain is hashed too, so two backends whose payloads encode alike do not collide.
        core::hash::Hash::hash(self.ops.domain, state);
        (self.ops.hash)(&*self.payload, state);
    }
}

#[cfg(feature = "bevy_reflect")]
impl core::fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}:", self.ops.domain)?;
        (self.ops.debug)(&*self.payload, f)
    }
}

/// Type data letting a stored [`DeviceId`] find its way back to a backend's own type.
///
/// Registered for you by
/// [`register_device_identity`](RegisterDeviceIdentity::register_device_identity).
#[cfg(feature = "bevy_reflect")]
#[derive(Clone)]
pub struct ReflectDeviceIdentity {
    domain: &'static str,
    // Only the deserializer rebuilds an identity from a loaded payload; without it this is a claim
    // on the domain name and nothing more.
    #[cfg(feature = "serialize")]
    from_payload: fn(&dyn bevy_reflect::PartialReflect) -> Option<DeviceId>,
}

#[cfg(feature = "bevy_reflect")]
impl ReflectDeviceIdentity {
    fn of<T: DeviceIdentity>() -> Self {
        Self {
            domain: T::DOMAIN,
            #[cfg(feature = "serialize")]
            from_payload: |value| {
                <T as bevy_reflect::FromReflect>::from_reflect(value).map(DeviceId::new)
            },
        }
    }

    /// The name identities of this type are stored under.
    pub fn domain(&self) -> &'static str {
        self.domain
    }
}

/// Declares a backend's [`DeviceIdentity`] type to the app, so identities of it can be stored and
/// read back.
///
/// A backend calls this once at startup for each identity type it produces. Without it, an identity
/// of that type still compares and hashes, but a saved one cannot be loaded: nothing knows which
/// type the stored [`DOMAIN`](DeviceIdentity::DOMAIN) refers to.
#[cfg(feature = "bevy_reflect")]
pub trait RegisterDeviceIdentity {
    /// Registers `T` for reflection and claims its `DOMAIN`.
    fn register_device_identity<T: DeviceIdentity>(&mut self) -> &mut Self;
}

#[cfg(feature = "bevy_reflect")]
impl RegisterDeviceIdentity for bevy_app::App {
    fn register_device_identity<T: DeviceIdentity>(&mut self) -> &mut Self {
        self.register_type::<T>();
        let registry = self
            .world()
            .resource::<bevy_ecs::reflect::AppTypeRegistry>();
        let mut registry = registry.write();
        if let Some(claimed) = registry
            .iter()
            .find_map(|registration| registration.data::<ReflectDeviceIdentity>())
            .filter(|claimed| claimed.domain == T::DOMAIN)
        {
            // Bound only to test for a claim; the warning carries the consequence.
            let _ = claimed;
            log::warn!(
                "the device identity domain `{}` is claimed by more than one type; a saved \
                 identity will resolve to whichever was registered first",
                T::DOMAIN
            );
        }
        registry
            .get_mut(core::any::TypeId::of::<T>())
            .expect("just registered")
            .insert(ReflectDeviceIdentity::of::<T>());
        drop(registry);
        self
    }
}

#[cfg(feature = "serialize")]
impl bevy_reflect::serde::SerializeWithRegistry for DeviceId {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
        registry: &bevy_reflect::TypeRegistry,
    ) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        // The domain is the stored name, taken straight off the payload's own ops. The body goes
        // through the *typed* serializer, which writes no type information — handing this to
        // `ReflectSerializer` instead is what would put a Rust type path in a player's save file.
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(
            self.ops.domain,
            &bevy_reflect::serde::TypedReflectSerializer::new(&*self.payload, registry),
        )?;
        map.end()
    }
}

#[cfg(feature = "serialize")]
impl<'de> bevy_reflect::serde::DeserializeWithRegistry<'de> for DeviceId {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
        registry: &bevy_reflect::TypeRegistry,
    ) -> Result<Self, D::Error> {
        deserializer.deserialize_map(DeviceIdVisitor { registry })
    }
}

#[cfg(feature = "serialize")]
struct DeviceIdVisitor<'a> {
    registry: &'a bevy_reflect::TypeRegistry,
}

#[cfg(feature = "serialize")]
impl<'de> serde::de::Visitor<'de> for DeviceIdVisitor<'_> {
    type Value = DeviceId;

    fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("a device identity: one entry keyed by its backend's domain")
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error as _;

        let Some(domain) = map.next_key::<alloc::string::String>()? else {
            return Err(A::Error::custom(
                "a device identity holds one entry, not none",
            ));
        };
        read_identity(&domain, &mut map, self.registry)
    }
}

/// Reads the payload half of a stored identity, once its domain has been taken off the map.
///
/// Shared by [`DeviceId`] and [`SavedDeviceId`], which differ only in whether an empty map is an
/// error or an answer.
#[cfg(feature = "serialize")]
fn read_identity<'de, A: serde::de::MapAccess<'de>>(
    domain: &str,
    map: &mut A,
    registry: &bevy_reflect::TypeRegistry,
) -> Result<DeviceId, A::Error> {
    use serde::de::Error as _;

    // Domain to concrete type, through the registry rather than through a type path. A scan,
    // because this runs once per stored identity at load time rather than per tick.
    let registration = registry
        .iter()
        .find(|registration| {
            registration
                .data::<ReflectDeviceIdentity>()
                .is_some_and(|claimed| claimed.domain == domain)
        })
        .ok_or_else(|| {
            A::Error::custom(alloc::format!(
                "no backend has claimed the device identity domain `{domain}`"
            ))
        })?;
    let claimed = registration
        .data::<ReflectDeviceIdentity>()
        .expect("matched on it above")
        .clone();

    let payload = map.next_value_seed(bevy_reflect::serde::TypedReflectDeserializer::new(
        registration,
        registry,
    ))?;
    (claimed.from_payload)(&*payload).ok_or_else(|| {
        A::Error::custom(alloc::format!(
            "the device identity domain `{domain}` did not accept its stored payload"
        ))
    })
}

/// A stored identity slot that may be empty.
///
/// A [`DeviceId`] says which device; this says "that device, or none yet", which is what a settings
/// field holding a pairing actually needs — a player who has not picked up a pad still has a row in
/// the file. Stored as the identity's own single entry, or as an empty table when there is none.
///
/// Use this rather than `Option<DeviceId>` in anything a settings layer writes. A reflected
/// `Option` serializes its empty case as `none`, which TOML has no way to spell, and a settings
/// crate that writes TOML will fail on it rather than leaving the field out.
#[cfg(feature = "serialize")]
#[derive(bevy_reflect::Reflect, Clone, Debug, Default, PartialEq)]
#[reflect(opaque)]
#[reflect(PartialEq, Debug)]
#[reflect(SerializeWithRegistry, DeserializeWithRegistry)]
pub struct SavedDeviceId(pub Option<DeviceId>);

#[cfg(feature = "serialize")]
impl bevy_reflect::serde::SerializeWithRegistry for SavedDeviceId {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
        registry: &bevy_reflect::TypeRegistry,
    ) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        match &self.0 {
            Some(device) => device.serialize(serializer, registry),
            None => serializer.serialize_map(Some(0))?.end(),
        }
    }
}

#[cfg(feature = "serialize")]
impl<'de> bevy_reflect::serde::DeserializeWithRegistry<'de> for SavedDeviceId {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
        registry: &bevy_reflect::TypeRegistry,
    ) -> Result<Self, D::Error> {
        deserializer.deserialize_map(SavedDeviceIdVisitor { registry })
    }
}

#[cfg(feature = "serialize")]
struct SavedDeviceIdVisitor<'a> {
    registry: &'a bevy_reflect::TypeRegistry,
}

#[cfg(feature = "serialize")]
impl<'de> serde::de::Visitor<'de> for SavedDeviceIdVisitor<'_> {
    type Value = SavedDeviceId;

    fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("a device identity keyed by its backend's domain, or an empty table for none")
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let Some(domain) = map.next_key::<alloc::string::String>()? else {
            return Ok(SavedDeviceId(None));
        };
        read_identity(&domain, &mut map, self.registry).map(|device| SavedDeviceId(Some(device)))
    }
}

// `Identity` is not gamepad-only: any backend's device can carry one.
#[cfg(any(feature = "gamepad", feature = "bevy_reflect"))]
use bevy_ecs::prelude::Component;
#[cfg(any(feature = "gamepad", feature = "bevy_reflect"))]
use core::ops::Deref;

#[cfg(feature = "gamepad")]
use bevy_ecs::entity::Entity;
#[cfg(feature = "gamepad")]
use bevy_ecs::lifecycle::{Add, Remove};
#[cfg(feature = "gamepad")]
use bevy_ecs::prelude::{Changed, Commands, On, Query, Res, Resource, Without};
#[cfg(feature = "gamepad")]
use bevy_input::gamepad::{
    AxisSettings, ButtonAxisSettings, ButtonSettings, Gamepad, GamepadAxis, GamepadSettings,
};
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
/// The default is the identity: centred at zero, wandering not at all, which is what an
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
    /// Deliberately does not rescale. At most one stage may, since otherwise a threshold would stop
    /// denoting any particular stick position, and that one is the binding's: the only threshold a
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
/// Empty by default, which reads as "every axis is honest": a game that never touches this gets
/// raw readings unchanged. Fill it from [`CalibrationSampling`], or [`set`](Self::set) a value
/// directly for a game that lets the player enter one.
///
/// Keyed by the backend's entity for the pad, so nothing here survives a reconnect.
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

/// Which manufacturer's conventions a connected gamepad follows, for prompts and glyphs that want
/// to say "A" on an Xbox pad and "Cross" on a PlayStation one rather than "South Button" on both.
///
/// `vendor_id` is `Option` and often absent — wasm, some Linux setups — so `Generic` is the
/// ordinary answer for an unrecognized or unreported pad, not an error.
#[cfg(feature = "gamepad")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadBrand {
    /// An Xbox controller.
    Xbox,
    /// A PlayStation controller.
    PlayStation,
    /// A Nintendo controller — a Switch Pro Controller or Joy-Con.
    Nintendo,
    /// Every other pad, and one Bevy could not identify.
    Generic,
}

#[cfg(feature = "gamepad")]
impl core::fmt::Display for GamepadBrand {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Xbox => "Xbox",
            Self::PlayStation => "PlayStation",
            Self::Nintendo => "Nintendo",
            Self::Generic => "Generic",
        })
    }
}

/// Resolves a connected gamepad's [`GamepadBrand`] from its `vendor_id`.
///
/// Seeded with the three current-generation console makers' USB vendor ids, not
/// SDL_GameControllerDB's full device list. [`insert`](Self::insert) extends the table for
/// hardware this crate does not ship pre-resolved.
#[cfg(feature = "gamepad")]
#[derive(Resource, Debug)]
pub struct GamepadBrands {
    by_vendor: HashMap<u16, GamepadBrand>,
}

#[cfg(feature = "gamepad")]
impl Default for GamepadBrands {
    fn default() -> Self {
        let mut by_vendor = HashMap::new();
        by_vendor.insert(0x045E, GamepadBrand::Xbox); // Microsoft
        by_vendor.insert(0x054C, GamepadBrand::PlayStation); // Sony
        by_vendor.insert(0x057E, GamepadBrand::Nintendo); // Nintendo
        Self { by_vendor }
    }
}

#[cfg(feature = "gamepad")]
impl GamepadBrands {
    /// Adds or replaces which brand a vendor id resolves to.
    pub fn insert(&mut self, vendor_id: u16, brand: GamepadBrand) {
        self.by_vendor.insert(vendor_id, brand);
    }

    /// Resolves a brand from a gamepad's vendor id, `Generic` if it is unknown or absent.
    pub fn resolve(&self, vendor_id: Option<u16>) -> GamepadBrand {
        vendor_id
            .and_then(|id| self.by_vendor.get(&id).copied())
            .unwrap_or(GamepadBrand::Generic)
    }
}

/// A connected gamepad's [`GamepadBrand`], resolved once and attached to its entity.
///
/// Query this instead of reading a gamepad's `vendor_id` and asking [`GamepadBrands`] yourself, so
/// that a pad from any backend answers the same way.
#[cfg(feature = "gamepad")]
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Brand(pub GamepadBrand);

#[cfg(feature = "gamepad")]
impl Deref for Brand {
    type Target = GamepadBrand;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Attaches [`Brand`] to a gamepad's entity as soon as it connects, resolved from its `vendor_id`
/// through [`GamepadBrands`].
///
/// Leaves an existing `Brand` alone, so inserting one yourself ahead of time overrides this for a
/// pad you know better than the vendor id table does.
#[cfg(feature = "gamepad")]
pub fn resolve_gamepad_brand(
    connected: On<Add<Gamepad>>,
    mut commands: Commands,
    gamepads: Query<&Gamepad, Without<Brand>>,
    brands: Res<GamepadBrands>,
) {
    let entity = connected.entity;
    if let Ok(gamepad) = gamepads.get(entity) {
        commands
            .entity(entity)
            .insert(Brand(brands.resolve(gamepad.vendor_id())));
    }
}

/// The keyboard and mouse, as a persistent identity.
///
/// Carries nothing, because there is nothing to carry: a machine has one keyboard as far as this
/// crate is concerned, so naming it is the whole of identifying it. Store one to remember that a
/// player was on the keyboard rather than a pad.
///
/// It follows that two players sharing one keyboard — one on the arrow keys, one on WASD — are not
/// distinguishable by this, and nothing here divides a keyboard between them.
///
/// Unlike a gamepad's, this identity is never attached to an entity and never has to be looked up —
/// [`DeviceHandle::KeyboardMouse`] is always available and always means the same device. A game
/// restoring a saved pairing acts on it at startup rather than waiting for the device to turn up.
#[cfg(feature = "bevy_reflect")]
#[derive(bevy_reflect::Reflect, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyboardMouseId;

#[cfg(feature = "bevy_reflect")]
impl DeviceIdentity for KeyboardMouseId {
    const DOMAIN: &'static str = "keyboard-mouse";
}

/// What Bevy's own gamepad backend can say about which device a pad is: the USB vendor and product
/// ids it reported when it connected.
///
/// **This names a model, not a unit.** Two identical controllers on the same table report the same
/// vendor and product id and cannot be told apart by it, so treat a match as a candidate rather
/// than an answer.
///
/// Not every platform reports these. They are absent on wasm and on some Linux setups, and a pad
/// that reports neither has no identity of this kind at all.
#[cfg(all(feature = "gamepad", feature = "bevy_reflect"))]
#[derive(bevy_reflect::Reflect, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GamepadModelId {
    /// The USB vendor id.
    pub vendor: u16,
    /// The USB product id.
    pub product: u16,
}

#[cfg(all(feature = "gamepad", feature = "bevy_reflect"))]
impl DeviceIdentity for GamepadModelId {
    const DOMAIN: &'static str = "gamepad-model";
}

#[cfg(all(feature = "gamepad", feature = "bevy_reflect"))]
impl GamepadModelId {
    /// The model id a connected pad reports, or `None` if it reports either half as absent.
    pub fn of(gamepad: &Gamepad) -> Option<Self> {
        Some(Self {
            vendor: gamepad.vendor_id()?,
            product: gamepad.product_id()?,
        })
    }
}

/// A connected device's persistent identity, resolved once and attached to its entity.
///
/// Query this rather than working an identity out from a gamepad's ids yourself, so that a device
/// from any backend answers the same way.
///
/// A device that cannot report an identity has no `Identity` at all, which is why this is a
/// component rather than a field.
#[cfg(feature = "bevy_reflect")]
#[derive(Component, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Identity(pub DeviceId);

#[cfg(feature = "bevy_reflect")]
impl Deref for Identity {
    type Target = DeviceId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Attaches [`Identity`] to a gamepad's entity as soon as it connects, built from the ids Bevy's
/// gamepad backend reported.
///
/// Leaves an existing `Identity` alone, so a backend that knows a device better than its USB ids do
/// can insert its own ahead of this and keep it.
#[cfg(all(feature = "gamepad", feature = "bevy_reflect"))]
pub fn resolve_gamepad_identity(
    connected: On<Add<Gamepad>>,
    mut commands: Commands,
    gamepads: Query<&Gamepad, Without<Identity>>,
) {
    let entity = connected.entity;
    if let Ok(gamepad) = gamepads.get(entity)
        && let Some(model) = GamepadModelId::of(gamepad)
    {
        commands
            .entity(entity)
            .insert(Identity(DeviceId::new(model)));
    }
}

/// A gamepad that is connected and whose input this crate will deliver.
///
/// Query it to enumerate the pads available right now, and observe `Add` and `Remove` on it to
/// learn when one arrives or goes away. It carries nothing, because the entity it sits on is
/// already the answer: [`DeviceHandle::Gamepad`] is built from that entity.
///
/// ```ignore
/// fn pads(pads: Query<Entity, With<ConnectedGamepad>>) {
///     for entity in &pads {
///         let device = DeviceHandle::Gamepad(entity);
///     }
/// }
/// ```
///
/// Bevy's own `Gamepad` component is a different thing, and not a substitute for this one. It holds
/// a pad's live button and axis readings, which only a backend feeding raw device messages can fill
/// in; a platform input service that reports finished actions has no such readings to offer, so a
/// pad it supplies would carry an empty one and read as though nobody were touching it. Enumerating
/// through this component instead is what lets one game work on either kind of backend.
///
/// Bevy's gamepad backend gets this attached for you. A backend of your own inserts it on the
/// entities it spawns, and removes it when a pad goes away.
#[cfg(feature = "gamepad")]
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConnectedGamepad;

/// Attaches [`ConnectedGamepad`] to a pad Bevy's own gamepad backend connected, so it enumerates
/// alongside any other backend's.
#[cfg(feature = "gamepad")]
pub fn mark_gamepad_connected(connected: On<Add<Gamepad>>, mut commands: Commands) {
    commands.entity(connected.entity).insert(ConnectedGamepad);
}

/// Removes [`ConnectedGamepad`] when Bevy's own gamepad backend loses a pad.
///
/// Watches the component rather than the entity because that backend keeps the entity alive across
/// a disconnect — it removes `Gamepad` and re-adds it on reconnect, so the entity outlives any
/// single connection and despawning is never the signal.
#[cfg(feature = "gamepad")]
pub fn mark_gamepad_disconnected(disconnected: On<Remove<Gamepad>>, mut commands: Commands) {
    // `try_` because a game is free to despawn a pad's entity outright, which removes `Gamepad` on
    // the way out and would leave this addressing something already gone.
    commands
        .entity(disconnected.entity)
        .try_remove::<ConnectedGamepad>();
}

/// Warns about gamepad settings this crate does not honour.
///
/// This crate reads raw gamepad messages, which Bevy emits before its own `GamepadSettings`
/// deadzones and thresholds are applied: a clamp applied below you cannot be undone above you, and
/// owning the whole chain is the only way a game can ask for a deadzone smaller than the one
/// someone underneath already applied. A game that configures `GamepadSettings` and expects it to
/// reach a binding would otherwise get silence, so this says so once.
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
// `ButtonSettings` and `AxisSettings` derive `PartialEq`; `ButtonAxisSettings` does not, so its
// three public fields are compared by hand. The per-control maps are tested for being populated at
// all rather than compared field by field against a default.
#[cfg(feature = "gamepad")]
fn is_customized(settings: &GamepadSettings) -> bool {
    let default_button_axis = ButtonAxisSettings::default();
    let moved_button_axis = &settings.default_button_axis_settings;

    settings.default_button_settings != ButtonSettings::default()
        || settings.default_axis_settings != AxisSettings::default()
        || (
            moved_button_axis.high,
            moved_button_axis.low,
            moved_button_axis.threshold,
        ) != (
            default_button_axis.high,
            default_button_axis.low,
            default_button_axis.threshold,
        )
        || !settings.axis_settings.is_empty()
        || !settings.button_settings.is_empty()
        || !settings.button_axis_settings.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "serialize")]
    use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

    /// Two backends' identity types, as different as the real ones are: one structural, one a
    /// single opaque handle.
    #[cfg(feature = "bevy_reflect")]
    #[derive(bevy_reflect::Reflect, Clone, Debug, PartialEq, Eq, Hash)]
    struct UsbDeviceId {
        vendor: u16,
        product: u16,
    }

    #[cfg(feature = "bevy_reflect")]
    impl DeviceIdentity for UsbDeviceId {
        const DOMAIN: &'static str = "usb";
    }

    /// The second backend also owns its stored form, which is what the docs tell a backend to do
    /// when its type may change — and what lets a handle above `i64::MAX` be stored at all.
    #[cfg(feature = "bevy_reflect")]
    #[derive(bevy_reflect::Reflect, Clone, Debug, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
    struct PlatformDeviceId(u64);

    #[cfg(feature = "serialize")]
    impl serde::Serialize for PlatformDeviceId {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&alloc::format!("{:016x}", self.0))
        }
    }

    #[cfg(feature = "serialize")]
    impl<'de> serde::Deserialize<'de> for PlatformDeviceId {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let text = <alloc::string::String as serde::Deserialize>::deserialize(deserializer)?;
            u64::from_str_radix(&text, 16)
                .map(Self)
                .map_err(serde::de::Error::custom)
        }
    }

    #[cfg(feature = "bevy_reflect")]
    impl DeviceIdentity for PlatformDeviceId {
        const DOMAIN: &'static str = "platform";
    }

    #[cfg(feature = "bevy_reflect")]
    #[test]
    fn an_identity_compares_and_hashes_as_its_own_payload_does() {
        use bevy_platform::collections::HashMap;

        let pad = UsbDeviceId {
            vendor: 0x054C,
            product: 0x05C4,
        };
        let same = DeviceId::new(pad.clone());
        let other = DeviceId::new(UsbDeviceId {
            vendor: 0x045E,
            product: 0x028E,
        });

        assert_eq!(DeviceId::new(pad.clone()), same);
        assert_ne!(same, other);
        assert_eq!(same.clone(), same);

        // The use this exists for: settings stored against an identity and found again.
        let mut stored: HashMap<DeviceId, f32> = HashMap::default();
        stored.insert(DeviceId::new(pad.clone()), 0.1);
        assert_eq!(stored.get(&same), Some(&0.1));
        assert_eq!(stored.get(&other), None);
    }

    /// The keyboard is a device too, and a player who joined on it has to come back to it. Its
    /// identity carries nothing because there is nothing to carry, which is the one case where an
    /// identity is equal to every other of its kind rather than to one piece of hardware.
    #[cfg(feature = "bevy_reflect")]
    #[test]
    fn the_keyboard_has_an_identity_and_it_is_always_the_same_one() {
        assert_eq!(
            DeviceId::new(KeyboardMouseId),
            DeviceId::new(KeyboardMouseId)
        );
        assert_eq!(DeviceId::new(KeyboardMouseId).domain(), "keyboard-mouse");
        assert_ne!(
            DeviceId::new(KeyboardMouseId),
            DeviceId::new(PlatformDeviceId(0))
        );
    }

    #[cfg(feature = "bevy_reflect")]
    #[test]
    fn identities_from_two_backends_never_collide() {
        // Payloads that could encode alike. The domain is what keeps them apart, in equality and in
        // the hash both, so one backend can never answer for another's device.
        let platform = DeviceId::new(PlatformDeviceId(7));
        let usb = DeviceId::new(UsbDeviceId {
            vendor: 0,
            product: 7,
        });

        assert_ne!(platform, usb);
        assert_eq!(platform.domain(), "platform");
        assert_eq!(usb.domain(), "usb");
    }

    #[cfg(feature = "bevy_reflect")]
    #[test]
    fn an_identity_hands_back_its_own_type_and_no_other() {
        let id = DeviceId::new(PlatformDeviceId(7));

        assert_eq!(id.get::<PlatformDeviceId>(), Some(&PlatformDeviceId(7)));
        assert_eq!(id.get::<UsbDeviceId>(), None);
        // Debug names the backend first, so a log line says which one an identity came from.
        assert!(alloc::format!("{id:?}").starts_with("platform:"));
    }

    /// A registry carrying both backends, built the way an app builds one.
    #[cfg(feature = "serialize")]
    fn registered() -> bevy_app::App {
        let mut app = bevy_app::App::new();
        app.register_device_identity::<UsbDeviceId>();
        app.register_device_identity::<PlatformDeviceId>();
        app
    }

    /// What a settings layer does with a stored identity: write it, read it, get the same one back
    /// — and never write the Rust path of a type into a player's file, so that moving the type
    /// between modules does not orphan what they saved.
    #[cfg(feature = "serialize")]
    #[test]
    fn a_stored_identity_round_trips_under_its_declared_domain() {
        use bevy_reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
        use serde::de::DeserializeSeed;

        let app = registered();
        let types = app
            .world()
            .resource::<bevy_ecs::reflect::AppTypeRegistry>()
            .read();
        let registration = types.get(core::any::TypeId::of::<DeviceId>()).unwrap();

        for (id, expected) in [
            (
                DeviceId::new(UsbDeviceId {
                    vendor: 0x054C,
                    product: 0x05C4,
                }),
                "[usb]\nvendor = 1356\nproduct = 1476\n",
            ),
            (
                // Above `i64::MAX`, which only the backend's own encoding can store.
                DeviceId::new(PlatformDeviceId(0x9214_0000_0000_0001)),
                "platform = \"9214000000000001\"\n",
            ),
        ] {
            let text = toml::to_string(&TypedReflectSerializer::new(&id, &types)).unwrap();
            assert_eq!(text, expected);

            let value: toml::Value = toml::from_str(&text).unwrap();
            let read = TypedReflectDeserializer::new(registration, &types)
                .deserialize(value)
                .expect("reads back");
            assert_eq!(
                <DeviceId as bevy_reflect::FromReflect>::from_reflect(&*read).unwrap(),
                id
            );
        }
    }

    /// The empty slot is the case `SavedDeviceId` exists for, so it is the half worth pinning down:
    /// written as an empty table, read back, and editable by hand.
    #[cfg(feature = "serialize")]
    #[test]
    fn an_empty_identity_slot_round_trips_as_an_empty_table() {
        use bevy_reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
        use serde::de::DeserializeSeed;

        let app = registered();
        let types = app
            .world()
            .resource::<bevy_ecs::reflect::AppTypeRegistry>()
            .read();
        let registration = types.get(core::any::TypeId::of::<SavedDeviceId>()).unwrap();

        // A settings layer writes a group's fields into a table, so the slot is exercised as one
        // named field rather than as a document of its own.
        for (slot, expected) in [
            // An empty section, which is how TOML spells a table with nothing in it.
            (SavedDeviceId(None), "[player]\n"),
            (
                SavedDeviceId(Some(DeviceId::new(PlatformDeviceId(7)))),
                "[player]\nplatform = \"0000000000000007\"\n",
            ),
        ] {
            let mut table = toml::map::Map::new();
            table.insert(
                "player".into(),
                toml::Value::try_from(TypedReflectSerializer::new(&slot, &types)).unwrap(),
            );
            let text = toml::to_string(&toml::Value::Table(table)).unwrap();
            assert_eq!(text, expected);

            let value: toml::Value = toml::from_str(&text).unwrap();
            let read = TypedReflectDeserializer::new(registration, &types)
                .deserialize(value.get("player").unwrap().clone())
                .expect("reads back");
            assert_eq!(
                <SavedDeviceId as bevy_reflect::FromReflect>::from_reflect(&*read).unwrap(),
                slot
            );
        }
    }

    /// A file written by a build that had a backend this one does not. Refusing is the point: the
    /// alternative is resolving a stored identity to whichever type happened to be registered, and
    /// answering for somebody else's device.
    #[cfg(feature = "serialize")]
    #[test]
    fn an_unclaimed_domain_is_refused_rather_than_guessed_at() {
        use bevy_reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
        use serde::de::DeserializeSeed;

        let written = {
            let app = registered();
            let types = app
                .world()
                .resource::<bevy_ecs::reflect::AppTypeRegistry>()
                .read();
            toml::to_string(&TypedReflectSerializer::new(
                &DeviceId::new(PlatformDeviceId(7)),
                &types,
            ))
            .unwrap()
        };

        // The same app, minus the backend that wrote it.
        let mut app = bevy_app::App::new();
        app.register_device_identity::<UsbDeviceId>();
        let types = app
            .world()
            .resource::<bevy_ecs::reflect::AppTypeRegistry>()
            .read();
        let registration = types.get(core::any::TypeId::of::<DeviceId>()).unwrap();

        let value: toml::Value = toml::from_str(&written).unwrap();
        let error = TypedReflectDeserializer::new(registration, &types)
            .deserialize(value)
            .expect_err("an unclaimed domain must not resolve to another backend's type");
        assert!(
            alloc::format!("{error}").contains("platform"),
            "the error should name the domain nobody claimed: {error}"
        );
    }

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

        // Outside it, the reading is corrected but not stretched.
        assert!((drifting.apply(0.5) - 0.4).abs() < 1e-6);
        assert!((drifting.apply(-0.5) - -0.6).abs() < 1e-6);

        // Recentring pushes full deflection past the end of the range, and it is clamped rather
        // than passed on.
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
        // Half the observed spread, widened by `REST_MARGIN`.
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

        // A step during which one stick never moves measures nothing about it.
        let mut sampling = CalibrationSampling::default();
        sampling.observe(pad, GamepadAxis::LeftStickY, 0.0);
        sampling.finish(&mut calibration);

        assert_eq!(
            calibration.get(pad, GamepadAxis::LeftStickX).center,
            0.1,
            "an axis that reported nothing was reset rather than left alone"
        );
    }

    // A pad arrives carrying default settings, so the warning has to be about a game that moved
    // them — firing on the default would mean warning every game that ever connects a gamepad.
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

        // The two global fields: setting a threshold for every button at once, rather than one
        // button at a time, is the ordinary way to configure `GamepadSettings`.
        let global_button = GamepadSettings {
            default_button_settings: ButtonSettings::new(0.9, 0.1).unwrap(),
            ..Default::default()
        };
        assert!(is_customized(&global_button));

        let mut global_button_axis = GamepadSettings::default();
        global_button_axis.default_button_axis_settings.threshold = 0.5;
        assert!(is_customized(&global_button_axis));
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn owner_for_finds_the_device_in_its_own_family() {
        let pad = bevy_ecs::entity::Entity::from_bits(1);
        let set =
            DeviceHandleSet::from_iter([DeviceHandle::KeyboardMouse, DeviceHandle::Gamepad(pad)]);
        assert_eq!(
            set.owner_for(DeviceFamily::Gamepad),
            Some(DeviceHandle::Gamepad(pad))
        );
        assert_eq!(
            set.owner_for(DeviceFamily::KeyboardMouse),
            Some(DeviceHandle::KeyboardMouse)
        );
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn brand_resolves_from_the_seeded_vendor_ids() {
        let brands = GamepadBrands::default();
        assert_eq!(brands.resolve(Some(0x045E)), GamepadBrand::Xbox);
        assert_eq!(brands.resolve(Some(0x054C)), GamepadBrand::PlayStation);
        assert_eq!(brands.resolve(Some(0x057E)), GamepadBrand::Nintendo);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn brand_is_generic_when_the_vendor_id_is_unknown_or_absent() {
        let brands = GamepadBrands::default();
        assert_eq!(brands.resolve(Some(0x1234)), GamepadBrand::Generic);
        assert_eq!(brands.resolve(None), GamepadBrand::Generic);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn an_app_can_override_or_extend_the_seeded_table() {
        let mut brands = GamepadBrands::default();
        // A pad this crate does not ship pre-resolved.
        brands.insert(0x2DC8, GamepadBrand::PlayStation); // 8BitDo, playing PlayStation-style
        assert_eq!(brands.resolve(Some(0x2DC8)), GamepadBrand::PlayStation);

        // The seeded table is a default, not a fixture — an app can also correct it.
        brands.insert(0x045E, GamepadBrand::Generic);
        assert_eq!(brands.resolve(Some(0x045E)), GamepadBrand::Generic);
    }

    #[cfg(all(feature = "gamepad", feature = "bevy_reflect"))]
    #[test]
    fn resolve_gamepad_identity_attaches_what_the_pad_reported() {
        use bevy_app::App;
        use bevy_input::InputPlugin;
        use bevy_input::gamepad::{GamepadConnection, GamepadConnectionEvent};

        let mut app = App::new();
        app.add_plugins(InputPlugin);
        app.add_observer(resolve_gamepad_identity);

        let reported = app.world_mut().spawn_empty().id();
        let silent = app.world_mut().spawn_empty().id();
        // A backend that knows this device better than its USB ids do, having said so first.
        let claimed = app
            .world_mut()
            .spawn(Identity(DeviceId::new(PlatformDeviceId(7))))
            .id();

        for (gamepad, vendor_id, product_id) in [
            (reported, Some(0x054C), Some(0x05C4)),
            // Absent on wasm and some Linux setups: no ids, so no identity of this kind.
            (silent, None, None),
            (claimed, Some(0x054C), Some(0x05C4)),
        ] {
            app.world_mut().write_message(GamepadConnectionEvent::new(
                gamepad,
                GamepadConnection::Connected {
                    name: "test pad".into(),
                    vendor_id,
                    product_id,
                },
            ));
        }
        app.update();

        assert_eq!(
            app.world().get::<Identity>(reported).map(|id| id.0.clone()),
            Some(DeviceId::new(GamepadModelId {
                vendor: 0x054C,
                product: 0x05C4
            }))
        );
        assert_eq!(app.world().get::<Identity>(silent), None);
        assert_eq!(
            app.world().get::<Identity>(claimed).map(|id| id.0.clone()),
            Some(DeviceId::new(PlatformDeviceId(7))),
            "an identity inserted ahead of the observer should stand"
        );
    }

    /// The pool follows a pad nobody has claimed, which is the case `DeviceDisconnected` cannot
    /// report: that is an entity event raised once per `Paired` holding the device, so an unclaimed
    /// pad going away signals nothing at all — and an unclaimed pad is exactly what a join screen
    /// is prompting for.
    ///
    /// Through `ActionMapPlugin` rather than by adding the observers here, so the wiring is under
    /// test alongside the behaviour.
    #[cfg(feature = "gamepad")]
    #[test]
    fn the_marker_follows_a_pad_with_no_pairing_behind_it() {
        use bevy_app::App;
        use bevy_input::InputPlugin;
        use bevy_input::gamepad::{GamepadConnection, GamepadConnectionEvent};

        let mut app = App::new();
        app.add_plugins(InputPlugin);
        app.add_plugins(crate::ActionMapPlugin);

        let pad = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(GamepadConnectionEvent::new(
            pad,
            GamepadConnection::Connected {
                name: "test pad".into(),
                vendor_id: None,
                product_id: None,
            },
        ));
        app.update();

        assert!(
            app.world().get::<ConnectedGamepad>(pad).is_some(),
            "a connected pad never entered the pool"
        );

        app.world_mut().write_message(GamepadConnectionEvent::new(
            pad,
            GamepadConnection::Disconnected,
        ));
        app.update();

        assert!(
            app.world().get::<ConnectedGamepad>(pad).is_none(),
            "a pad left without leaving the pool"
        );
        // The entity outlives the connection, which is why a pool reads the marker rather than the
        // entity.
        assert!(app.world().get_entity(pad).is_ok());
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn resolve_gamepad_brand_attaches_it_once_per_connection() {
        use bevy_app::App;
        use bevy_input::InputPlugin;
        use bevy_input::gamepad::{GamepadConnection, GamepadConnectionEvent};

        let mut app = App::new();
        app.add_plugins(InputPlugin);
        app.insert_resource(GamepadBrands::default());
        app.add_observer(resolve_gamepad_brand);

        let known = app.world_mut().spawn_empty().id();
        let unreported = app.world_mut().spawn_empty().id();
        // A pad this crate would otherwise call `Xbox` — a game that knows better inserts its own
        // `Brand` ahead of the connection event, and the observer must leave it standing.
        let overridden = app.world_mut().spawn(Brand(GamepadBrand::Nintendo)).id();

        for (gamepad, vendor_id) in [
            (known, Some(0x045E)),
            (unreported, None),
            (overridden, Some(0x045E)),
        ] {
            app.world_mut().write_message(GamepadConnectionEvent::new(
                gamepad,
                GamepadConnection::Connected {
                    name: "test pad".into(),
                    vendor_id,
                    product_id: None,
                },
            ));
        }
        app.update();

        assert_eq!(
            app.world().get::<Brand>(known),
            Some(&Brand(GamepadBrand::Xbox))
        );
        assert_eq!(
            app.world().get::<Brand>(unreported),
            Some(&Brand(GamepadBrand::Generic))
        );
        assert_eq!(
            app.world().get::<Brand>(overridden),
            Some(&Brand(GamepadBrand::Nintendo))
        );
    }
}
