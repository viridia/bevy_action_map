#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![no_std]

//! Input action mapping for Bevy.
//!
//! Use this crate when you want to turn physical input into gameplay actions, then read those
//! actions back in your systems. It covers device data, input frames, action mapping, and the
//! player-facing presentation layer.
//!
//! Start with [`action`] to define your actions and contexts, declare what drives them with
//! [`binding`], and put a context on an entity with [`context`]. Use [`present`] and [`mapping`]
//! when you want to show players what is bound.
//!
//! ```rust
//! use bevy_action_map::prelude::*;
//!
//! #[derive(InputAction)]
//! #[action(path = "gameplay.jump", output = bool, intent = Button)]
//! struct Jump;
//!
//! #[derive(InputContext)]
//! #[context(path = "gameplay.on_foot", tick = Fixed)]
//! struct OnFoot;
//! ```

extern crate self as bevy_action_map;

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

// L0
pub mod device;

// L1
pub mod frame;

// L2
pub mod action;
pub mod binding;
pub mod condition;
pub mod context;
pub mod eval;
pub mod event;
pub mod plan;
pub mod player;

// L3
pub mod capture;
pub mod inspect;
pub mod mapping;
pub mod overrides;
pub mod present;

pub mod backend;

#[cfg(feature = "focus")]
#[cfg_attr(docsrs, doc(cfg(feature = "focus")))]
pub mod focus;

/// System sets for the two stages of the input pipeline.
///
/// Order your own systems against these when you need to run at a specific point relative to
/// input. [`Sample`](ActionMapSystems::Sample) collects device messages into the input frame;
/// [`Evaluate`](ActionMapSystems::Evaluate) maps that frame onto action state.
///
/// Sampling runs in `PreUpdate`, after Bevy's own input systems. Evaluation runs in `PreUpdate`
/// for render-tick contexts and in `FixedPreUpdate` for fixed-tick ones, so a system reading
/// actions from `Update` or `FixedUpdate` always sees state that is current for its own schedule.
#[derive(bevy_ecs::schedule::SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActionMapSystems {
    /// Collects raw device messages into the input frame.
    Sample,
    /// Maps the input frame onto action state.
    Evaluate,
    /// Delivers what changed to observers.
    Dispatch,
    /// Reads the frame on behalf of a live rebinding capture.
    ///
    /// Between [`Sample`](ActionMapSystems::Sample) and
    /// [`Evaluate`](ActionMapSystems::Evaluate), which is what lets a capture take a control before
    /// any context acts on it.
    Capture,
}

/// The plugin entry point for the mapping layer.
///
/// Add this alongside your other plugins, then declare contexts with
/// [`add_context`](context::ActionMapAppExt::add_context). It installs the input frame sampler if
/// you have not added [`InputFramePlugin`](frame::InputFramePlugin) yourself, and orders context
/// evaluation after sampling so a system reading actions never sees a stale frame.
pub struct ActionMapPlugin;

impl bevy_app::Plugin for ActionMapPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        use bevy_ecs::schedule::IntoScheduleConfigs;

        #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
        if !app.is_plugin_added::<frame::InputFramePlugin>() {
            app.add_plugins(frame::InputFramePlugin);
        }

        app.init_resource::<binding::ButtonThreshold>();
        app.init_resource::<eval::ConsumedControls>();
        app.init_resource::<capture::ReservedControls>();
        // Always present, so that whatever draws prompts can watch it from the first frame. Its
        // absence would mean nothing — unlike `PromptDevice`, where absence is the game not having
        // said which device it speaks for, and where a default would be a guess.
        app.init_resource::<present::PromptGeneration>();

        // After the release below, or a capture's claim would be cleared the moment it was made.
        app.add_systems(
            bevy_app::PreUpdate,
            capture::run_captures.in_set(ActionMapSystems::Capture),
        );

        // Two clearing points, per Design §5.2. The frame's starts everything from nothing; the
        // fixed one lets a schedule that runs several times decide afresh each run while what
        // `PreUpdate` claimed still stands.
        app.add_systems(
            bevy_app::PreUpdate,
            eval::release_consumed_controls.before(ActionMapSystems::Capture),
        );
        app.add_systems(
            bevy_app::FixedPreUpdate,
            eval::release_consumed_in::<bevy_app::FixedPreUpdate>
                .before(ActionMapSystems::Evaluate),
        );

        // Conditions and rate conversions are defined in simulated seconds (R9.6), so a clock is
        // not optional. `DefaultPlugins` brings one; a headless app or a test may not have.
        if !app.is_plugin_added::<bevy_time::TimePlugin>() {
            app.add_plugins(bevy_time::TimePlugin);
        }

        app.configure_sets(
            bevy_app::PreUpdate,
            (
                ActionMapSystems::Capture.after(ActionMapSystems::Sample),
                ActionMapSystems::Evaluate.after(ActionMapSystems::Capture),
                ActionMapSystems::Dispatch.after(ActionMapSystems::Evaluate),
            ),
        );
        // Fixed contexts never sample — the frame is filled once per render frame — but their
        // observers still have to run after their evaluation.
        app.configure_sets(
            bevy_app::FixedPreUpdate,
            ActionMapSystems::Dispatch.after(ActionMapSystems::Evaluate),
        );
    }
}

/// The action-map prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
pub mod prelude {
    pub use crate::action::{
        ActionId, ActionOutput, ActionState, ActionValue, ChannelShape, InputAction, InputContext,
        Intent, Phase, TickDomain,
    };
    pub use crate::{ActionMapPlugin, ActionMapSystems};
    // `InputContextBuilder` is deliberately absent: `add_context` hands one to a closure, so its
    // type is inferred and never written. Import it from `binding` to name it in a signature.
    #[cfg(feature = "gamepad")]
    pub use crate::binding::Stick;
    #[cfg(any(feature = "keyboard", feature = "gamepad"))]
    pub use crate::binding::{AxisButtons, DirectionalButtons};
    // `MouseMove` is ungated because `BindingSource::MouseMotion` is.
    pub use crate::binding::{ButtonThreshold, CompassPoints, Control, DeadZone, MouseMove, Part};
    pub use crate::capture::{
        CaptureSession, Captured, Conflict, ControlClass, Overlap, Refused, RefusedReason,
        ReservedControls, conflicts, conflicts_pending,
    };
    pub use crate::condition::{Condition, ConditionDescriptor, ConditionKind, Verdict};
    pub use crate::context::{ActionMapAppExt, Actions, ActionsQuery, InputContextState, Obstacle};
    pub use crate::event::{Canceled, Completed, Fired, Started};
    pub use crate::frame::{InputFrame, RawEvent, TimedRawEvent, Timestamp};
    pub use crate::mapping::{
        Capacity, Follower, Mapping, MappingKey, Rebinding, Scheme, mappings,
    };
    pub use crate::present::{
        BindingTable, ControlOrigin, Prompt, PromptDevice, PromptGeneration, PromptScope, Prompts,
    };
    // The derives share their names with the traits above, which is fine — a derive macro and a
    // trait live in different namespaces. Without these, a glob import of this prelude gives you
    // the trait and leaves `#[derive(InputAction)]` unresolved.
    pub use bevy_action_map_macros::{InputAction, InputContext};

    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    pub use crate::frame::{InputFramePlugin, sample_input};
}

pub use bevy_action_map_macros::{InputAction, InputContext};

/// Names the derives reach for, so that using one does not mean importing them.
#[doc(hidden)]
pub mod __macro_exports {
    pub use bevy_ecs::component::{Component, Mutable, StorageType};
}
