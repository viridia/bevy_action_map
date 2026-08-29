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
