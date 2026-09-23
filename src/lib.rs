#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![no_std]

//! Input action mapping for Bevy.
//!
//! Declare the things your game reacts to, such as `Jump`, `Move` and `Fire`, as Rust types. Bind
//! the keyboard, mouse and gamepad controls that drive them, and read the result in a system
//! without naming the device that produced it. The same declarations drive a settings screen: what
//! is bound, what the player may rebind, and a prompt that stays correct after they do.
//!
//! # Quick start
//!
//! ```rust
//! use bevy::prelude::*;
//! use bevy_action_map::prelude::*;
//!
//! #[derive(InputAction)]
//! #[action(path = "gameplay.jump", output = bool, intent = Button)]
//! struct Jump;
//!
//! #[derive(InputContext)]
//! #[context(path = "gameplay.on_foot", tick = Render)]
//! struct OnFoot;
//!
//! fn read_jump(input: ContextActions<OnFoot>) {
//!     if input.fired::<Jump>() {
//!         // ...
//!     }
//! }
//!
//! let mut app = App::new();
//! app.add_plugins((MinimalPlugins, ActionMapPlugin));
//! app.add_context::<OnFoot>(|context| {
//!     context.bind::<Jump>(KeyCode::Space);
//! });
//! app.add_systems(Update, read_jump);
//! ```
//!
//! # Concepts
//!
//! ## Actions and contexts
//!
//! An [action] is a type, not a value. `#[derive(InputAction)]` gives it the Rust type your
//! gameplay reads (`bool`, `f32`, `Vec2`, …) and an [`ActionIntent`](action::ActionIntent) saying
//! what that value means: a button, a continuous axis, a direction to keep moving, or a delta that
//! already happened this frame. A mouse delta and a stick position are both `Vec2`, and the intent
//! is what stops a binding from treating one as the other. Every action also declares a stable
//! `path` such as `"gameplay.jump"`, which is what a settings file stores; it does not have to
//! match the Rust type name, and should not change when the type is renamed.
//!
//! A [context] groups the actions that are active together: on foot, in a vehicle, in a menu.
//! [`add_context`](context::ActionMapAppExt::add_context) declares one and assigns it to an entity,
//! and that entity carries the live state for every action in it. Each player in a local
//! multiplayer game gets their own instance on their own entity; when more than one may be live at
//! once, read them with [`ActionsQuery`](context::ActionsQuery) instead of
//! [`ContextActions`](context::ContextActions).
//!
//! A context can be always active, or gated with
//! [`active_in_state`](binding::InputContextBuilder::active_in_state) or
//! [`active_if`](binding::InputContextBuilder::active_if). Contexts also carry a priority
//! (`#[context(priority = …)]`), so a menu that consumes the arrow keys for navigation keeps them
//! from also moving the player in the gameplay context beneath it.
//!
//! ## Bindings, modifiers, and conditions
//!
//! A [binding] pairs a control with the action it drives, and reads left to right as a pipeline:
//!
//! ```ignore
//! context.bind::<Move>(Stick::Left).dead_zone(DeadZone::radial(0.15));
//! ```
//!
//! Several bindings can feed one action, such as a key and a gamepad button for the same jump, or
//! four keys combined into one `Move` composite. When two bindings in a context share a control,
//! the more specific one wins: `Ctrl+S` beats a plain `S` without either binding knowing the other
//! exists.
//!
//! A keyboard binding names either where a key is or what it types. A
//! [`KeyCode`](bevy_input::keyboard::KeyCode) is a position, which is what movement wants: `WASD`
//! is a shape under the left hand and should keep that shape on an AZERTY board. A
//! [`LogicalKey`](binding::LogicalKey) is the character the player's own layout produces, which is
//! what a shortcut wants, so `Ctrl+Z` reaches the key a French player reads as `z`.
//!
//! [Modifiers](binding) reshape the raw value on its way to the action: dead zones, response
//! curves, scale, negate, swizzle, clamping, and rate conversion (turning a stick's *position* into
//! the same per-frame *delta* a mouse reports). [Conditions](condition) decide *when* a binding
//! counts as firing. Without one, a binding fires whenever its control is away from rest;
//! `.hold(0.4)` waits for the control to stay down that long, and `.multi_tap(2, 0.3)` waits for
//! two presses inside the window. Durations are measured in the context's own simulated seconds, so
//! a paused clock pauses them and a fixed-tick replay reproduces them exactly.
//!
//! A binding can carry several conditions, each one of three kinds. At least one *explicit*
//! condition must hold, every *implicit* one must, and any *blocking* one that holds vetoes the
//! binding. So `.press()` and `.hold(0.5)` together mean "either a press or a long hold."
//!
//! ## Reading actions
//!
//! Read an action by polling [`ContextActions`](context::ContextActions) or
//! [`ActionsQuery`](context::ActionsQuery) in a system (`input.value::<Move>()`,
//! `input.fired::<Jump>()`, `input.phase::<Jump>()`), or by observing a transition [event] such as
//! [`Fired`](event::Fired), [`Started`](event::Started), [`Completed`](event::Completed), or
//! [`Canceled`](event::Canceled), delivered to the entity holding the context. Polling suits
//! `FixedUpdate` simulation code that wants an answer every tick regardless of whether anything
//! changed; observing suits a one-shot reaction, such as a UI confirm or a sound effect.
//!
//! Every action has an [`ActionPhase`](action::ActionPhase) each tick (`Idle`, `Started`,
//! `Building`, `Fired`, `Firing`, `Completed`, `Canceled`), so a hold that has just begun is never
//! confused with one still charging, and a UI can show a charge meter from the hold's first frame
//! instead of reconstructing that edge from a boolean. When an action does not fire and it is not
//! obvious why, [`why_not`](context::ContextActions::why_not) answers with the specific
//! [`ActionObstacle`](context::ActionObstacle): an inactive context, a higher-priority consumer, a
//! longer chord winning, an unmet condition, or a device that is not this player's.
//!
//! ## Tick domains
//!
//! A context declares a [`TickDomain`](action::TickDomain), `Render` or `Fixed`, and evaluates once
//! per tick of whichever one it picked: a camera-look context on the render tick, a gameplay
//! context on the fixed tick, both reading the same devices without either one guessing at the
//! other's timing. The two do not run in step, but a fixed-tick context still sees every press and
//! release exactly once, however many times `FixedUpdate` runs between rendered frames, including
//! none.
//!
//! A context's domain is fixed when it is declared, so an action needed at both rates is bound in
//! two contexts, one per domain.
//!
//! ## Local co-op
//!
//! Two players on one machine means two devices, and neither should see the other's input. A
//! [`Paired`](player::Paired) component, sibling to the context, narrows a context instance to the
//! devices it names; a context with no `Paired` reads every device.
//! [`apply_overrides_for`](overrides::apply_overrides_for) applies overrides to one paired instance
//! rather than to all of them, so two players on identical pads can rebind independently, and
//! neither one's changes become the default a third player inherits.
//!
//! A device gets into a `Paired` through a [join] gesture: declare join as an ordinary action on a
//! listener context spawned once per available device, each `Paired` to its own. The press then
//! arrives on an entity that already knows which device it came from. See [`join`] for the worked
//! recipe, and for what happens when two players press on the same tick.
//!
//! ## Presentation
//!
//! The binding API above is a developer's model. Dead zones and response curves are implementation
//! detail nobody rebinding "move forward" should have to think about. Marking a binding
//! [`mappable`](binding::BindingBuilder::mappable) adds it to a smaller model built for
//! presentation: a named [mapping] with an ordered list of slots ("Primary", "Secondary"), which a
//! settings screen can walk without knowing anything else about your actions or bindings.
//!
//! From there the crate can list what is bound, run an interactive [capture] for a new control
//! (with conflict detection against everything else in the context, and reserved controls a game
//! never wants handed out), and apply the result as a live [override](overrides) that cancels
//! whatever was in flight and takes effect immediately. [Presets](preset) apply a whole named
//! arrangement of mappings at once, for a game that ships alternate control families (`Southpaw`,
//! `Classic`) rather than leaving a player to rebind every row by hand.
//!
//! The controls a player uses to reach that screen, and to find their way around it, have to
//! survive whatever they rebind. Mark them [`reserved`](binding::BindingBuilder::reserved), which
//! stops the player moving them and stops capture handing their controls to any other mapping.
//! Leaving a binding unmappable does only the first, and a key that opens the screen but also fires
//! the gun is as much a trap as one that opens nothing. Give the screen a way back to the shipped
//! controls too, with [`reset_all`](overrides::Overrides::reset_all), for whatever reserving did
//! not cover.
//!
//! An on-screen [prompt](present) ("Press W") stays correct across a rebind because it is derived
//! from the same data the settings screen edits, not typed out separately. The control it names
//! also has a stable, storage-safe string form, so a save file and a localization catalogue can
//! both key off it without depending on any platform's names for its buttons. A prompt shown to the
//! player can instead name a pad's buttons the way that pad does, resolved from the device actually
//! connected: "Cross" on a DualSense, "A" on an Xbox pad.
//!
//! Not every setting on a controls screen is a control. [Tunables](mapping) cover the rest: look
//! sensitivity, invert-Y, whether crouch is a hold or a toggle. They are declared beside the
//! bindings and travel in the same overrides, and a binding that reads one sees the player's
//! current value without the game passing it along.
//!
//! ## Saving what a player changed
//!
//! [`Overrides`](overrides::Overrides) is the live set of changes sitting on top of what the game
//! declared, and it is what a settings screen edits. To persist it, convert it to
//! [`SavedOverrides`](overrides::SavedOverrides), a plain, owned, reflectable type whose field
//! names become the file's keys, so a `Reflect`-based settings layer can write it without calling
//! into this crate. Where the bytes go is up to the app; the crate does no file I/O.
//!
//! Loading reports what it cannot restore instead of dropping it. Saved names are resolved against
//! what the game declares *now*, and anything that no longer resolves, such as a renamed mapping or
//! a control this build has no feature for, comes back as a problem the game can show the player
//! rather than silently vanishing from their settings.
//!
//! # Feature flags
//!
//! | Flag          | Default | Enables                                                          |
//! | ------------- | :-----: | ----------------------------------------------------------------- |
//! | `std`         |   yes   | The standard library. Off for `no_std` + `alloc` targets.         |
//! | `libm`        |         | A software math backend, for `no_std` builds without `std`'s.     |
//! | `keyboard`    |   yes   | Keyboard keys as a binding input.                                |
//! | `mouse`       |   yes   | Mouse buttons and motion as a binding input.                     |
//! | `gamepad`     |   yes   | Gamepad buttons and axes as a binding input.                     |
//! | `touch`       |         | Planned: touch as a binding input. Gates the dependency only.    |
//! | `bevy_reflect`|   yes   | Runtime reflection, needed to register custom modifiers and conditions. |
//! | `serialize`   |         | `serde` support for saving and loading binding overrides.         |
//! | `state`       |   yes   | A context's activation can follow a `bevy_state` state.           |

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
pub mod join;
pub mod mapping;
pub mod overrides;
pub mod present;
pub mod preset;

pub mod backend;

/// System sets for the stages of the input pipeline.
///
/// Order your own systems against these when you need to run at a specific point relative to
/// input. They run in this order: [`Sample`](ActionMapSystems::Sample) collects device messages
/// into the input frame, [`Capture`](ActionMapSystems::Capture) offers it to a live rebinding
/// session, [`Evaluate`](ActionMapSystems::Evaluate) maps it onto action state, and
/// [`Dispatch`](ActionMapSystems::Dispatch) delivers what changed to observers.
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
    /// Ahead of evaluation, so a capture takes a control before any context acts on it.
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
        // `InputFramePlugin` only exists to sample devices, so a build with none of them never adds
        // it — but `run_captures` and context evaluation read `InputFrame` regardless.
        app.init_resource::<frame::InputFrame>();

        // Here rather than beside the gamepad one in `InputFramePlugin`: `DeviceHandle::KeyboardMouse`
        // exists whatever the device features say, so its identity does too.
        #[cfg(feature = "bevy_reflect")]
        {
            use device::RegisterDeviceIdentity;
            app.register_device_identity::<device::KeyboardMouseId>();
        }

        // So a `Reflect`-based settings layer can load a saved override set with nothing registered
        // but the app's own settings group. A value type the deserializer cannot find is not an
        // error a caller sees — it is the whole field, silently dropped, and a game that loses every
        // rebind on restart with no diagnostic anywhere.
        #[cfg(feature = "serialize")]
        app.register_type::<overrides::SavedOverrides>();

        app.init_resource::<binding::ButtonThreshold>();
        app.init_resource::<eval::ConsumedControls>();
        app.init_resource::<eval::ExclusionCeiling>();
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

        // Not inside `evaluate_context`; see the system's own doc.
        #[cfg(feature = "gamepad")]
        app.add_systems(
            bevy_app::PreUpdate,
            player::watch_gamepad_connections.in_set(ActionMapSystems::Dispatch),
        );

        // Two clearing points, per TD5.2. The frame's starts everything from
        // nothing; the fixed one lets a schedule that runs several times decide afresh each run
        // while what `PreUpdate` claimed still stands. The exclusion ceiling (TD5.3) clears at the
        // same point as the frame's consumption release and nowhere else — see `ExclusionCeiling`.
        app.add_systems(
            bevy_app::PreUpdate,
            (
                eval::release_consumed_controls,
                eval::reset_exclusion_ceiling,
            )
                .before(ActionMapSystems::Capture),
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
        ActionId, ActionIntent, ActionOutput, ActionPhase, ActionState, ActionValue, ChannelShape,
        InputAction, InputContext, TickDomain,
    };
    pub use crate::{ActionMapPlugin, ActionMapSystems};
    // `InputContextBuilder` is deliberately absent: `add_context` hands one to a closure, so its
    // type is inferred and never written. Import it from `binding` to name it in a signature.
    #[cfg(feature = "gamepad")]
    pub use crate::binding::Stick;
    // Also in `bevy::prelude`, so a glob import of both resolves to the same item. Without this,
    // `bind::<Jump>(KeyCode::Space)` — the crate's own quick start — does not compile on its own.
    #[cfg(any(feature = "keyboard", feature = "gamepad"))]
    pub use crate::binding::{AxisButtons, DirectionalButtons};
    #[cfg(feature = "keyboard")]
    pub use crate::binding::{LogicalKey, ModifierKey};
    #[cfg(feature = "keyboard")]
    pub use bevy_input::keyboard::KeyCode;
    // `MouseMove` is ungated because `BindingInput::MouseMotion` is.
    pub use crate::backend::AuthorityValues;
    pub use crate::binding::{
        BindingPart, ButtonThreshold, CompassPoints, Control, DeadZone, MouseMove,
    };
    pub use crate::capture::{
        CaptureSession, ConflictOverlap, ControlCaptured, ControlClass, MappingConflict,
        ReservedControls, conflicts, conflicts_pending,
    };
    pub use crate::condition::{Condition, ConditionDescriptor, ConditionKind, ConditionState};
    pub use crate::context::{
        ActionMapAppExt, ActionObstacle, ActionsQuery, ContextActions, InputContextState,
    };
    #[cfg(feature = "gamepad")]
    pub use crate::device::ConnectedGamepad;
    pub use crate::device::DeviceFamily;
    pub use crate::event::{Canceled, ClassBinding, ClassFired, Completed, Fired, Started};
    pub use crate::frame::{FrameTimestamp, InputFrame, RawEvent, TimedRawEvent};
    pub use crate::join::is_claimed;
    pub use crate::mapping::{
        ActionMapping, BoundSlot, Follower, MappingKey, RebindPolicy, Tunable, TunableValue,
        declared_mappings, declared_tunables, mappings, tunables,
    };
    pub use crate::present::{
        BindingTable, ControlOrigin, Glyph, GlyphTier, Prompt, PromptDevice, PromptGeneration,
        PromptScope, Prompts, resolve_glyph,
    };
    // The derives share their names with the traits above, which is fine — a derive macro and a
    // trait live in different namespaces. Without these, a glob import of this prelude gives you
    // the trait and leaves `#[derive(InputAction)]` unresolved.
    pub use bevy_action_map_macros::{InputAction, InputContext};

    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    pub use crate::frame::{InputFramePlugin, sample_input};
}

pub use bevy_action_map_macros::{InputAction, InputContext};

/// Names the types the derive macros need, so that using a derive does not mean importing them.
#[doc(hidden)]
pub mod __macro_exports {
    pub use bevy_ecs::component::{Component, Mutable, StorageType};
    pub use bevy_ecs::lifecycle::ComponentHook;

    pub use crate::context::warn_if_undeclared;
}
