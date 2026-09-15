//! The declaration API, and the record it writes.

use alloc::vec::Vec;
use bevy_platform::sync::Arc;
use core::marker::PhantomData;

use crate::action::{ActionId, ActionIntent, InputAction};
use crate::condition::{BindingCondition, Condition};
use crate::event::{Dispatch, dispatch_for};
use crate::mapping::{always_reports_bool, mappings_of, tunables_of, widest};

#[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
use super::control::ButtonControl;
use super::control::{BindingInput, IntoBindingInput};
use super::modifier::{BindingModifier, CompassPoints, DeadZone, Modifier};

/// One binding as the setup closure declared it: one `.bind` call, plus whatever was chained onto
/// it. This is what the compiled plan is built from.
// Cloned when an override is applied: the variant is the authored set with some inputs rewritten,
// and the authored set has to stay intact so a later diff still has defaults to diff against.
#[derive(Clone)]
pub(crate) struct BindingSpec {
    pub(crate) action: ActionId,
    // Carried from the action type at bind time: the plan keys state by `ActionId`, which does not
    // reach back to the type, and folding several bindings into one action needs the intent. The
    // path is here so plan-build diagnostics can name the action a mistake is in.
    pub(crate) intent: ActionIntent,
    pub(crate) path: &'static str,
    pub(crate) category: Option<&'static str>,
    // The only place the concrete action type survives bind time. Everything downstream works in
    // slots, which cannot name a generic event.
    pub(crate) dispatch: Dispatch,
    pub(crate) input: BindingInput,
    pub(crate) modifiers: Vec<BindingModifier>,
    pub(crate) conditions: Vec<BindingCondition>,
    pub(crate) consume: bool,
    // `None` for a binding declared `private`, and for one declared `follows` — the first has no
    // mapping and the second rides someone else's.
    pub(crate) mapping: Option<MappingDecl>,
    // Set by `follows`: the mapping this binding rides instead of declaring one.
    pub(crate) follows: Option<FollowsDecl>,
    // What a player may tune on this binding, if anything. At most one; a `Vec` is the change to
    // make if a binding ever needs two.
    pub(crate) tunable: Option<TunableDecl>,
    // Whether the controls this binding reads are withheld from capture across their family.
    pub(crate) reserved: bool,
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    pub(crate) chord: Vec<ButtonControl>,
}

/// One action as [`InputContextBuilder::delegate`] declared it: named, and left to an authority
/// outside this crate.
///
/// Deliberately not a `BindingSpec`: with no input to modify, condition, consume or list, attaching
/// any of those to a delegated action is unrepresentable rather than a mistake to diagnose.
#[derive(Clone)]
pub(crate) struct DelegatedSpec {
    pub(crate) action: ActionId,
    pub(crate) intent: ActionIntent,
    pub(crate) path: &'static str,
    pub(crate) dispatch: Dispatch,
}

/// One class binding as [`InputContextBuilder::bind_class`] declared it.
///
/// Deliberately not a `BindingSpec`: a class binding has no input to modify, no chord, no mapping,
/// and nothing to combine.
pub(crate) struct ClassBindingSpec {
    pub(crate) action_path: &'static str,
    pub(crate) filter: crate::capture::ClassFilter,
    pub(crate) consume: bool,
    pub(crate) dispatch: crate::event::ClassDispatch,
}

/// What a binding contributes to the presentation list.
///
/// Every binding has one of these unless it was declared `private`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MappingDecl {
    /// Replaces the action's path in the derived key. `None` derives from the action.
    pub(crate) prefix: Option<&'static str>,
    /// The most controls a player may put in the mapping this binding contributes to.
    ///
    /// Declared per binding but resolved per mapping, by `widest`. Meaningless unless
    /// `rebind_policy` is `Here`, since nothing can add a control to a mapping the player cannot
    /// change. `None` means unlimited.
    pub(crate) capacity: Option<usize>,
    /// Whether the player may change it, or is only being shown what it does.
    pub(crate) rebind_policy: crate::mapping::RebindPolicy,
}

/// What a player may tune on one binding.
///
/// `tunable_dead_zone` and `hold_or_toggle` both declare one of these: a named, typed value that
/// overwrites one field of one modifier already on the binding, applied the same way a rebind is —
/// by rewriting `modifiers[modifier_index]` and recompiling (R19.11).
#[derive(Clone, Copy, Debug)]
pub(crate) struct TunableDecl {
    /// A localization key (R19.14), chosen by the game rather than derived — unlike a mapping's
    /// key, nothing about a modifier names itself.
    pub(crate) key: &'static str,
    /// Which entry of this binding's `modifiers` the tunable's value rewrites.
    pub(crate) modifier_index: usize,
    /// The shape and the game's own declared value — bounds included, since a player adjusts the
    /// value but never the range it is adjusted within.
    pub(crate) default: crate::mapping::TunableValue,
}

/// The action whose mapping a binding rides.
///
/// Named rather than resolved at declaration time, because the mapping's name may have been changed
/// with `mappable_as` and a binding cannot see its neighbours at the point it is written.
/// Resolution is `leader_of`, over the whole context.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FollowsDecl {
    pub(crate) action: ActionId,
    /// The target's declared path, for the diagnostic that names it when resolution fails.
    pub(crate) path: &'static str,
}

impl MappingDecl {
    /// What a binding gets by saying nothing: listed, and not changeable.
    const fn listed() -> Self {
        Self {
            prefix: None,
            capacity: Some(1),
            rebind_policy: crate::mapping::RebindPolicy::Fixed,
        }
    }
}

/// Configures the binding that was just declared, one chained call at a time.
pub struct BindingBuilder<'a, C> {
    builder: &'a mut InputContextBuilder<C>,
    index: usize,
}

/// Configures the class binding that was just declared, one chained call at a time.
///
/// Deliberately not [`BindingBuilder`]: a class binding has nothing to run a modifier or a
/// condition against, so this exposes only what actually applies to one.
pub struct ClassBindingBuilder<'a, C> {
    builder: &'a mut InputContextBuilder<C>,
    index: usize,
}

impl<C> ClassBindingBuilder<'_, C> {
    /// Takes this class binding's controls, so that lower-priority contexts do not see them.
    ///
    /// The generalization of a plain binding's [`consume`](BindingBuilder::consume) from one
    /// control to every member of the class this binding watches — a focused text field claiming
    /// character-producing keys away from gameplay is the motivating case.
    pub fn consume(self) -> Self {
        self.builder.class_bindings[self.index].consume = true;
        self
    }
}

impl<'a, C> BindingBuilder<'a, C> {
    fn push_modifier(&mut self, modifier: BindingModifier) {
        self.builder.bindings[self.index].modifiers.push(modifier);
    }

    fn push_condition(&mut self, condition: BindingCondition) {
        self.builder.bindings[self.index].conditions.push(condition);
    }

    /// Fires on the press rather than for as long as the control is held.
    pub fn press(mut self) -> Self {
        self.push_condition(BindingCondition::Press);
        self
    }

    /// Fires when the control is let go.
    pub fn release(mut self) -> Self {
        self.push_condition(BindingCondition::Release);
        self
    }

    /// Requires the control to still be held, alongside whatever else this binding asks for.
    pub fn down(mut self) -> Self {
        self.push_condition(BindingCondition::Down);
        self
    }

    /// Fires once the control has been held for `duration` seconds, and keeps firing after.
    ///
    /// Letting go early cancels rather than firing, so an action can show how far along it is and
    /// then take it back.
    pub fn hold(mut self, duration: f32) -> Self {
        self.push_condition(BindingCondition::Hold {
            duration,
            one_shot: false,
        });
        self
    }

    /// Fires once, when the control has been held for `duration` seconds.
    pub fn hold_once(mut self, duration: f32) -> Self {
        self.push_condition(BindingCondition::Hold {
            duration,
            one_shot: true,
        });
        self
    }

    /// Fires on release, if the control was held for at least `duration` seconds first.
    pub fn hold_and_release(mut self, duration: f32) -> Self {
        self.push_condition(BindingCondition::HoldAndRelease { duration });
        self
    }

    /// Fires on release, if the control was held no longer than `max_duration` seconds.
    pub fn tap(mut self, max_duration: f32) -> Self {
        self.push_condition(BindingCondition::Tap { max_duration });
        self
    }

    /// Fires after `count` taps, each within `max_gap` seconds of the one before.
    pub fn multi_tap(mut self, count: u16, max_gap: f32) -> Self {
        self.push_condition(BindingCondition::MultiTap { count, max_gap });
        self
    }

    /// Fires every `interval` seconds while the control is held, starting immediately.
    pub fn pulse(mut self, interval: f32) -> Self {
        self.push_condition(BindingCondition::Pulse {
            interval,
            immediate: true,
        });
        self
    }

    /// Fires whenever the value differs from what it was on the tick before.
    ///
    /// The cheapest way to turn a control that reports a position into one that reports events. A
    /// stick held off centre is off centre every tick, so a binding on it fires every tick; this
    /// narrows that to the ticks on which something actually moved.
    ///
    /// Returning to rest is a change like any other, so a reader that only cares about the
    /// direction the player chose should ignore a value at rest.
    ///
    /// ```ignore
    /// // One fire per direction entered, and auto-repeat while it is held.
    /// context
    ///     .bind::<Navigate>(Stick::Left)
    ///     .dead_zone(DeadZone::radial(0.5))
    ///     .compass(CompassPoints::Four)
    ///     .on_change()
    ///     .pulse(0.15);
    /// ```
    pub fn on_change(mut self) -> Self {
        self.push_condition(BindingCondition::Change);
        self
    }

    /// Requires another control to be held as well.
    ///
    /// This is how `Ctrl+S` is spelled: bind the action to `S`, and add `Ctrl` with this. Call it
    /// more than once for a longer chord.
    ///
    /// ```ignore
    /// context.bind::<Save>(KeyCode::KeyS).with(KeyCode::ControlLeft);
    /// context.bind::<SaveAs>(KeyCode::KeyS).with(KeyCode::ControlLeft).with(KeyCode::ShiftLeft);
    /// ```
    ///
    /// **A longer chord wins.** When several bindings read the same control, the one requiring the
    /// most held alongside it takes the control and the shorter ones do not fire — so `Ctrl+S` does
    /// not also trigger a plain `S` binding, and `Ctrl+Shift+S` does not trigger either of the
    /// other two. Nothing has to be declared for that; it follows from the lengths.
    #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
    pub fn with(self, control: impl Into<ButtonControl>) -> Self {
        self.builder.bindings[self.index].chord.push(control.into());
        self
    }

    /// Takes this binding's controls, so that lower-priority contexts do not see them.
    ///
    /// Opt-in per binding rather than per context, because a context usually wants to claim only
    /// some of what it reads: a menu should take `Escape` from the game behind it, while a global
    /// screenshot key on `F12` goes on working whatever is on screen.
    ///
    /// The claim lasts for the rest of the frame, and reaches only contexts that evaluate after
    /// this one — which means later in priority order, and never backwards across a tick domain.
    /// A control is claimed only on the ticks the binding actually fires.
    ///
    /// An action can ask for this on all of its bindings at once with `#[action(consume)]`, which
    /// is usually what a menu action wants. This is the same switch, one binding at a time.
    pub fn consume(self) -> Self {
        self.builder.bindings[self.index].consume = true;
        self
    }

    /// Leaves this binding's controls for lower-priority contexts to see.
    ///
    /// Only needed to make an exception of one binding on an action declared with
    /// `#[action(consume)]` — say a menu action that claims its keyboard key but shares the
    /// gamepad button with the game behind it.
    pub fn without_consuming(self) -> Self {
        self.builder.bindings[self.index].consume = false;
        self
    }

    /// Lets the player rebind this.
    ///
    /// Declares a mapping for the binding — one for a single control, and one per part for a
    /// composite, so a movement binding becomes four rows a player can change independently and the
    /// composite itself is never shown.
    ///
    /// Rebinding is opt-in per binding rather than per action, which is what lets a game offer its
    /// keyboard bindings for remapping while leaving the gamepad to the console or to Steam:
    ///
    /// ```ignore
    /// controls.bind::<Jump>(KeyCode::Space).mappable();
    /// controls.bind::<Jump>(GamepadButton::South);   // listed, but the player cannot change it
    /// ```
    ///
    /// Each mapping is named by the action's path plus the part — `gameplay.move.up` — which is a
    /// localization key rather than text to show. Use [`mappable_as`](Self::mappable_as) where that
    /// name would collide or where a catalogue already calls it something else.
    ///
    /// **Declaring two of these for one action in one family is how you ship a default primary and
    /// secondary.** They derive the same key, so they are one row holding two controls rather than
    /// two rows; the mapping's capacity grows to fit them without being asked. Use
    /// [`mappable_upto`](Self::mappable_upto) to leave a slot for a control the player adds that
    /// the game does not ship a default for.
    ///
    /// ```ignore
    /// controls.bind::<Jump>(KeyCode::Space).mappable();
    /// controls.bind::<Jump>(KeyCode::KeyJ).mappable();   // the same row, second slot
    /// ```
    pub fn mappable(self) -> Self {
        self.declare_mapping(None, Some(1))
    }

    /// Keeps this binding out of the presentation list entirely.
    ///
    /// Listing is the default, because a player is entitled to see what their controls do and a
    /// screen that shows only the rebindable half has holes in it. This is the exception: a binding
    /// that is genuinely the game's own business and would only confuse a controls screen.
    ///
    /// Use it where a binding is an implementation detail of another one — a second reading of a
    /// control that already appears under a different name — rather than a control the player
    /// operates.
    ///
    /// ```ignore
    /// // `Afterburner` is the throttle held down, on the same keys `Thrust` already shows.
    /// controls.bind::<Afterburner>(KeyCode::KeyW).hold(0.75).private();
    /// ```
    ///
    /// # Panics
    ///
    /// If the same binding was also declared `mappable`. One binding cannot be both hidden from the
    /// player and rebindable by them.
    pub fn private(self) -> Self {
        assert!(
            !self.builder.bindings[self.index]
                .mapping
                .is_some_and(|decl| decl.rebind_policy.is_rebindable()),
            "a binding cannot be both `mappable` and `private`: one says the player may change it, \
             the other says they may not see it"
        );
        self.builder.bindings[self.index].mapping = None;
        self
    }

    /// Lets the player rebind this, under a name of your choosing.
    ///
    /// As [`mappable`](Self::mappable), with the given key in place of the action's path — so a
    /// composite declared `mappable_as("gameplay.strafe")` has mappings `gameplay.strafe.up` and
    /// its three neighbours. Use it when two would otherwise derive the same key, which happens
    /// when one action is bound in two contexts.
    pub fn mappable_as(self, key: &'static str) -> Self {
        self.declare_mapping(Some(key), Some(1))
    }

    /// Lets the player rebind this, and put up to `count` controls in the mapping.
    ///
    /// What a "primary and secondary" screen declares when the game ships only one default and
    /// leaves the other slot empty. A mapping never ends up narrower than the defaults it holds, so
    /// this raises a ceiling rather than setting one.
    ///
    /// ```ignore
    /// controls.bind::<Fire>(KeyCode::ControlLeft).mappable_upto(2);
    /// ```
    ///
    /// # Panics
    ///
    /// If `count` is zero. A mapping with no room is a binding the player cannot change, which is
    /// what leaving `mappable` off already says.
    pub fn mappable_upto(self, count: usize) -> Self {
        assert!(
            count > 0,
            "a mapping needs room for at least one control; leave `mappable` off instead"
        );
        self.declare_mapping(None, Some(count))
    }

    /// Lets the player rebind this, with no limit on how many controls the mapping holds.
    ///
    /// For a program whose command set is large and open enough that its shortcuts cannot be laid
    /// out in a table written in advance — an editor or a tool, where the screen grows an "add
    /// shortcut" button. A game almost always wants a fixed number of slots instead.
    pub fn mappable_any(self) -> Self {
        self.declare_mapping(None, None)
    }

    fn declare_mapping(self, prefix: Option<&'static str>, capacity: Option<usize>) -> Self {
        let existing = self.builder.bindings[self.index].mapping;
        assert!(
            self.builder.bindings[self.index].follows.is_none(),
            "a binding cannot be both `follows` and `mappable`: one rides another action's mapping, \
             the other gives it one of its own"
        );
        assert!(
            existing.is_some(),
            "a binding cannot be both `private` and `mappable`: one says the player may not see it, \
             the other says they may change it"
        );
        self.builder.bindings[self.index].mapping = Some(MappingDecl {
            // A later call names the mapping; `mappable_as(..).mappable_upto(2)` must not silently
            // drop the name, and neither order should surprise.
            prefix: prefix.or(existing.and_then(|decl| decl.prefix)),
            capacity: match existing {
                Some(decl) => widest(decl.capacity, capacity),
                None => capacity,
            },
            // Every one of this method's callers is a `mappable*`, so reaching here is the author
            // asking for the upgrade from the listed-but-fixed default.
            rebind_policy: crate::mapping::RebindPolicy::Here,
        });
        self
    }

    /// Withholds this binding's controls from capture, everywhere in its family.
    ///
    /// The control that opens the rebinding screen is the case this exists for. It is not
    /// rebindable, so a player cannot move it away, and no *other* mapping can capture it, so it
    /// cannot be quietly shadowed by something bound over the top of it. Without the second half
    /// the first is worth little: a screen you can still open but whose controls now do two things
    /// is the same trap arriving by a different door.
    ///
    /// It stays *listed*, which is usually what you want — a player looking for the key that opens
    /// this screen should be able to find it written down. Add [`private`](Self::private) to keep
    /// it out of the list as well.
    ///
    /// ```ignore
    /// controls.bind::<OpenSettings>(KeyCode::F1).reserved();
    /// controls.bind::<OpenSettings>(GamepadButton::Select).reserved();
    /// ```
    ///
    /// Reserving is per family, because that is the scope a control is unambiguous in: reserving
    /// `F1` says nothing about the gamepad, and the pad binding above is what reserves `Select`.
    ///
    /// Capture refuses a reserved control out loud, with
    /// [`RefusedReason::Reserved`](crate::capture::RefusedReason::Reserved), rather than ignoring
    /// it — a player who has just pressed it is owed the reason. That is what separates this from
    /// [`excluding`](crate::capture::CaptureSession::excluding), which is silent because the
    /// control is busy doing its normal job.
    ///
    /// Reserving and [`mappable`](Self::mappable) contradict each other, and declaring both is
    /// refused when the context is declared.
    pub fn reserved(self) -> Self {
        self.builder.bindings[self.index].reserved = true;
        self
    }

    /// Adds an application-defined condition.
    pub fn when<K: Condition>(mut self, condition: K) -> Self {
        self.push_condition(BindingCondition::Custom(Arc::new(condition)));
        self
    }

    /// Adds a deadzone.
    ///
    /// ```ignore
    /// context.bind::<Move>(Stick::Left).dead_zone(DeadZone::radial(0.15));
    /// ```
    pub fn dead_zone(mut self, dead_zone: DeadZone) -> Self {
        self.push_modifier(BindingModifier::DeadZone(dead_zone));
        self
    }

    /// Lets the player adjust this binding's deadzone within `range`, as a named tunable rather
    /// than a rebinding row — the mechanism a stick's deadzone amount uses, since a stick is bound
    /// whole and has no per-mapping rebinding to offer instead.
    ///
    /// ```ignore
    /// context.bind::<Move>(Stick::Left)
    ///     .dead_zone(DeadZone::radial(0.15))
    ///     .tunable_dead_zone("gameplay.move.stick_deadzone", 0.0..=0.5);
    /// ```
    ///
    /// # Panics
    ///
    /// If [`dead_zone`](Self::dead_zone) was not declared first on the same binding, or if this
    /// binding already has a tunable.
    pub fn tunable_dead_zone(
        self,
        key: &'static str,
        range: core::ops::RangeInclusive<f32>,
    ) -> Self {
        let value = match self.builder.bindings[self.index].modifiers.last() {
            Some(BindingModifier::DeadZone(dead_zone)) => dead_zone.lower,
            _ => panic!(
                "`tunable_dead_zone` needs a `dead_zone` declared first on the same binding, so \
                 there is a deadzone for it to adjust"
            ),
        };
        self.declare_tunable(
            key,
            crate::mapping::TunableValue::Range {
                value,
                min: *range.start(),
                max: *range.end(),
            },
        )
    }

    fn declare_tunable(self, key: &'static str, default: crate::mapping::TunableValue) -> Self {
        assert!(
            self.builder.bindings[self.index].tunable.is_none(),
            "a binding may declare at most one tunable"
        );
        let modifier_index = self.builder.bindings[self.index].modifiers.len() - 1;
        self.builder.bindings[self.index].tunable = Some(TunableDecl {
            key,
            modifier_index,
            default,
        });
        self
    }

    /// Adds a scale modifier.
    pub fn scale(mut self, factor: f32) -> Self {
        self.push_modifier(BindingModifier::Scale(factor));
        self
    }

    /// Adds a negate modifier.
    pub fn negate(mut self) -> Self {
        self.push_modifier(BindingModifier::Negate);
        self
    }

    /// Adds an x/y swizzle modifier.
    pub fn swizzle(mut self) -> Self {
        self.push_modifier(BindingModifier::Swizzle);
        self
    }

    /// Adds a clamp modifier.
    pub fn clamp(mut self, min: f32, max: f32) -> Self {
        self.push_modifier(BindingModifier::Clamp { min, max });
        self
    }

    /// Adds a magnitude-clamp modifier.
    pub fn clamp_magnitude(mut self) -> Self {
        self.push_modifier(BindingModifier::ClampMagnitude);
        self
    }

    /// Adds a rescale modifier, mapping `min..max` onto `0..1`.
    pub fn rescale(mut self, min: f32, max: f32) -> Self {
        self.push_modifier(BindingModifier::Rescale { min, max });
        self
    }

    /// Adds a response-curve modifier.
    pub fn curve(mut self, power: f32) -> Self {
        self.push_modifier(BindingModifier::Curve(power));
        self
    }

    /// Reads this control as a rate, and converts it to the movement it caused this tick.
    ///
    /// A stick says how fast; a mouse says how far. They are different quantities, and adding them
    /// is the reason a look control can feel different at different frame rates. This is the
    /// conversion between them: `scale` is how far a fully deflected control should move the action
    /// in one second, and what comes out is the distance covered since the last tick.
    ///
    /// ```ignore
    /// // Both drive the same look action, in the same units.
    /// context.bind::<Look>(MouseMove);
    /// context.bind::<Look>(Stick::Right).dead_zone(DeadZone::radial(0.12)).per_second(180.0);
    /// ```
    ///
    /// A control that already reports a displacement cannot be read as a rate, so this is refused
    /// on one.
    pub fn per_second(mut self, scale: f32) -> Self {
        self.push_modifier(BindingModifier::PerSecond(scale));
        self
    }

    /// Rounds a direction to the nearest compass point, discarding how far it was pushed.
    ///
    /// A stick reports a position; a menu wants a direction. This is the conversion between them:
    /// what comes out is a unit vector along one of four or eight compass points, or rest, and
    /// nothing in between. Pair it with a deadzone, which is what decides how far the stick has to
    /// travel before it counts as pointing anywhere at all.
    ///
    /// ```ignore
    /// // Move the selection one item per direction the stick is pushed in.
    /// context
    ///     .bind::<Navigate>(Stick::Left)
    ///     .dead_zone(DeadZone::radial(0.5))
    ///     .compass(CompassPoints::Four)
    ///     .on_change();
    /// ```
    ///
    /// Rounding on its own does not stop the action from firing every tick — the stick stays off
    /// centre for as long as the player holds it. [`on_change`](Self::on_change) is what makes it
    /// fire once per direction entered.
    ///
    /// A one-dimensional value has two compass points rather than four, and which one it is on is
    /// its sign, so this rounds one to -1, 0 or 1. A boolean has no direction to round, and neither
    /// does a 3D value; both pass through untouched.
    pub fn compass(mut self, points: CompassPoints) -> Self {
        self.push_modifier(BindingModifier::Compass(points));
        self
    }

    /// Adds a custom modifier.
    pub fn custom<M: Modifier>(mut self, modifier: M) -> Self {
        self.push_modifier(BindingModifier::Custom(Arc::new(modifier)));
        self
    }
}

/// Builder used by [`crate::context::ActionMapAppExt::add_context`].
pub struct InputContextBuilder<C> {
    bindings: Vec<BindingSpec>,
    class_bindings: Vec<ClassBindingSpec>,
    delegated: Vec<DelegatedSpec>,
    // Installed against the `App` once the context has been declared. `None` leaves the context
    // live from the moment an entity carries it; see `active_if`, which lives in `context` because
    // everything it touches does.
    pub(crate) activation: Option<crate::context::Activation>,
    _marker: PhantomData<C>,
}

impl<C> Default for InputContextBuilder<C> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
            class_bindings: Vec::new(),
            delegated: Vec::new(),
            activation: None,
            _marker: PhantomData,
        }
    }
}

impl<C> InputContextBuilder<C> {
    fn push_binding<A: InputAction>(&mut self, input: BindingInput) -> BindingBuilder<'_, C> {
        self.bindings.push(BindingSpec {
            action: A::id(),
            intent: A::INTENT,
            path: A::PATH,
            category: A::CATEGORY,
            dispatch: dispatch_for::<A>,
            input,
            modifiers: Vec::new(),
            conditions: Vec::new(),
            // The action's default, which a binding can then make an exception of either way.
            consume: A::CONSUMES,
            mapping: Some(MappingDecl::listed()),
            follows: None,
            tunable: None,
            reserved: false,
            #[cfg(any(feature = "keyboard", feature = "mouse", feature = "gamepad"))]
            chord: Vec::new(),
        });
        let index = self.bindings.len() - 1;
        BindingBuilder {
            builder: self,
            index,
        }
    }

    /// Binds an action to an input value.
    ///
    /// An action may be bound more than once — a keyboard key and a gamepad button, a stick and
    /// the movement keys. Every binding for an action contributes to the same value, combined
    /// according to the action's [`ActionIntent`]:
    ///
    /// - `Button`, `Analog1` and `Directional2` take the **strongest** contribution, so pushing the
    ///   stick further wins over tapping a key, and either of two buttons fires the action. Equal
    ///   contributions resolve in the order the bindings were declared.
    /// - `Delta2` **sums** its contributions, because a delta is a displacement and two devices
    ///   moving at once should move the action by both.
    ///
    /// The control has to be one the action can actually use. A control reports on a channel of a
    /// particular [`ChannelShape`](crate::action::ChannelShape), the action declares an
    /// [`ActionIntent`](crate::action::ActionIntent), and a binding between
    /// two that do not fit — a single button asked to give a direction, a mouse asked to hold a
    /// position — is refused when the context is declared. [`ActionIntent::accepts`] has the table.
    ///
    /// ```ignore
    /// context.bind::<Jump>(KeyCode::Space);
    /// context.bind::<Jump>(GamepadButton::South);
    /// context.bind::<Move>(DirectionalButtons::wasd());
    /// context.bind::<Move>(Stick::Left).dead_zone(DeadZone::radial(0.15));
    /// context.bind::<Look>(MouseMove);
    /// ```
    pub fn bind<A: InputAction>(&mut self, input: impl IntoBindingInput) -> BindingBuilder<'_, C> {
        self.push_binding::<A>(input.into_binding_input())
    }

    /// Declares `Follower` as riding every one of `Leader`'s bindings, one for one.
    ///
    /// Use it where an action deliberately shares a control with another — tap to dodge and hold
    /// to sprint, or a throttle that opens up when it is held down. `Leader` must already have
    /// its bindings declared: this reads them off, generates one matching binding of `Follower` per
    /// device `Leader` reads, and runs `configure` on each. The player rebinds *the control*, once,
    /// and every action riding it moves with it.
    ///
    /// ```ignore
    /// controls.bind::<Thrust>(KeyCode::KeyW).mappable();
    /// controls.bind::<Thrust>(KeyCode::ArrowUp).mappable();
    /// // Same two keys, without retyping either of them.
    /// controls.follow::<Afterburner, Thrust>(|binding| binding.hold(0.75));
    /// ```
    ///
    /// Following a binding the player cannot change is allowed and useful — it keeps the duplicate
    /// off the screen — and leaves nothing to rewrite. Calling this before `Leader` has every
    /// device bound is not an error and not a mistake to catch: it is what lets a follower ride
    /// only some of `Leader`'s devices, by naming `Leader`'s bindings so far rather than all of
    /// them ever declared.
    ///
    /// # Panics
    ///
    /// If `Leader` has no bindings declared yet, or if `Follower` and `Leader` are the same action.
    pub fn follow<Follower: InputAction, Leader: InputAction>(
        &mut self,
        configure: impl Fn(BindingBuilder<'_, C>) -> BindingBuilder<'_, C>,
    ) {
        assert!(
            Follower::id() != Leader::id(),
            "`{}` cannot follow its own action: it would be riding the mapping it is declaring",
            Follower::PATH
        );
        let inputs: Vec<BindingInput> = self
            .bindings
            .iter()
            .filter(|binding| binding.action == Leader::id())
            .map(|binding| binding.input)
            .collect();
        assert!(
            !inputs.is_empty(),
            "`{}` has no bindings yet for `{}` to follow — declare `{}` first",
            Leader::PATH,
            Follower::PATH,
            Leader::PATH
        );
        for input in inputs {
            let binding = self.push_binding::<Follower>(input);
            binding.builder.bindings[binding.index].mapping = None;
            binding.builder.bindings[binding.index].follows = Some(FollowsDecl {
                action: Leader::id(),
                path: Leader::PATH,
            });
            configure(binding);
        }
    }

    /// Lets the player choose, once for `A`, between holding its controls and pressing one to
    /// toggle it on and off.
    ///
    /// Declared once for the whole action rather than chained onto one binding: "does `A` support
    /// toggling" is a fact about the action, not about which control happens to drive it, and a
    /// game with several bindings for `A` — a primary key and a secondary, say — almost always
    /// wants every one of them to answer the same way. This finds them all rather than asking you
    /// to name each one and trust yourself to repeat the same key correctly on every one.
    ///
    /// Only a binding whose control is a genuine press, with nothing analog to lose, is eligible —
    /// a key or a mouse button always is; a gamepad button is only when `A`'s own intent is
    /// [`Button`](crate::action::ActionIntent::Button), since the same control reads as a
    /// continuous fraction for anything else (a trigger driving an analog action), and toggling
    /// that would flatten it to on/off. A stick, an axis, mouse motion, or a composite are never
    /// eligible — there is no single press for any of them to toggle. Every eligible binding
    /// shares one latch: press any of them, release, press another, and the action reads one
    /// consistent state throughout — never one control turning it on while a different one turns
    /// it back off.
    ///
    /// Held is the default; nothing changes until a player (or a preset) turns toggle mode on.
    /// Downstream conditions read whatever the modifier chain produced, so `.down()` on a toggled
    /// binding reads "is the latch on" rather than "is the control physically held" — no condition
    /// needs to know which mode is in effect.
    ///
    /// ```ignore
    /// controls.bind::<Thrust>(GamepadButton::RightTrigger2);
    /// controls.bind::<Thrust>(KeyCode::KeyW).mappable();
    /// controls.bind::<Thrust>(KeyCode::ArrowUp).mappable();
    /// controls.hold_or_toggle::<Thrust>("gameplay.thrust.hold_or_toggle");
    /// ```
    ///
    /// # Ordering
    ///
    /// Reads `A`'s bindings as declared *so far* — the same rule [`follow`](Self::follow) follows.
    /// Call it after every binding of `A` you want it to reach, not before.
    ///
    /// # Panics
    ///
    /// If no binding of `A` declared so far is eligible — nothing yet bound, or every input so far
    /// is analog.
    pub fn hold_or_toggle<A: InputAction>(&mut self, key: &'static str) {
        let mut touched = 0usize;
        for binding in &mut self.bindings {
            if binding.action != A::id() || !always_reports_bool(&binding.input, binding.intent) {
                continue;
            }
            binding
                .modifiers
                .push(BindingModifier::Toggle { active: false });
            assert!(
                binding.tunable.is_none(),
                "`{}` already has a tunable; a binding may declare at most one",
                A::PATH
            );
            binding.tunable = Some(TunableDecl {
                key,
                modifier_index: binding.modifiers.len() - 1,
                default: crate::mapping::TunableValue::Bool(false),
            });
            touched += 1;
        }
        assert!(
            touched > 0,
            "`hold_or_toggle::<{}>` found no eligible binding — call it after every binding of \
             `{}` you want it to reach, and check whether any of them are analog (a trigger \
             feeding a non-`Button` intent has nothing to toggle)",
            A::PATH,
            A::PATH
        );
    }

    /// Binds to every control a [`ControlClass`](crate::capture::ControlClass) names, rather than
    /// to one control.
    ///
    /// Where a plain [`bind`](Self::bind) reads one control you name, this reads whichever member
    /// of the class shows up. It fires once per matching, otherwise-unclaimed event, carrying that
    /// event untouched; there is no value to combine and nothing to hold between ticks, so it skips
    /// modifiers, conditions and the presentation mapping list entirely.
    ///
    /// ```ignore
    /// struct AnyPress;
    /// impl ClassBinding for AnyPress {
    ///     const PATH: &'static str = "ui.any_press";
    /// }
    ///
    /// controls.bind_class::<AnyPress>(ControlClass::AnyButton);
    /// ```
    ///
    /// A control already named by a plain binding in this context never reaches a class binding,
    /// however it is declared — see [`bind`](Self::bind)'s doc for how several bindings on one
    /// action combine; a class binding does not compete in that the way a chord does, it yields.
    /// For keys that produce text rather than a fixed shape, use
    /// [`bind_characters`](Self::bind_characters) instead.
    pub fn bind_class<A: crate::event::ClassBinding>(
        &mut self,
        class: crate::capture::ControlClass,
    ) -> ClassBindingBuilder<'_, C> {
        self.push_class_binding::<A>(crate::capture::ClassFilter::Shape(class))
    }

    /// Binds to every keyboard key whose event carries text, once IME composition and dead keys are
    /// accounted for — the mechanism a focused text field uses to claim character-producing keys
    /// without the app enumerating them.
    ///
    /// Not a [`ControlClass`](crate::capture::ControlClass): which key this matches is a property
    /// of the *event*, not of the control's identity — the same key is a dead key on one press and
    /// a plain letter on the next — so there is no fixed set of controls to name here the way
    /// [`bind_class`](Self::bind_class) does.
    ///
    /// ```ignore
    /// struct TypedCharacter;
    /// impl ClassBinding for TypedCharacter {
    ///     const PATH: &'static str = "ui.typed_character";
    /// }
    ///
    /// controls.bind_characters::<TypedCharacter>().consume();
    /// ```
    pub fn bind_characters<A: crate::event::ClassBinding>(&mut self) -> ClassBindingBuilder<'_, C> {
        self.push_class_binding::<A>(crate::capture::ClassFilter::Characters)
    }

    /// Hands one action to an authority outside this crate, instead of binding controls to it.
    ///
    /// A platform's own input service is the usual reason: Steam Input owns the binding screen, the
    /// conflict rules and the glyphs, and hands the game a value per action rather than a control to
    /// map. A network peer's actions and a scripted agent's arrive the same way.
    ///
    /// The action still fires, completes and cancels on the edges of the value it is given, so
    /// gameplay code and observers read it exactly as they read a bound one. What it does not have
    /// is controls: no modifiers, no conditions, no consumption, and no row on your own rebinding
    /// screen — all of which belong to whoever owns the action now. Binding the same action in the
    /// same context is refused, since only one of the two can be the authority.
    ///
    /// Values arrive through [`AuthorityValues`](crate::backend::AuthorityValues) on the context's
    /// entity.
    ///
    /// ```ignore
    /// app.add_context::<Paddle>(|paddle| {
    ///     paddle.bind::<Move>(Stick::Left);
    ///     paddle.delegate::<Serve>();
    /// });
    /// ```
    pub fn delegate<A: InputAction>(&mut self) {
        if self.delegated.iter().any(|spec| spec.action == A::id()) {
            return;
        }
        self.delegated.push(DelegatedSpec {
            action: A::id(),
            intent: A::INTENT,
            path: A::PATH,
            dispatch: dispatch_for::<A>,
        });
    }

    fn push_class_binding<A: crate::event::ClassBinding>(
        &mut self,
        filter: crate::capture::ClassFilter,
    ) -> ClassBindingBuilder<'_, C> {
        self.class_bindings.push(ClassBindingSpec {
            action_path: A::PATH,
            filter,
            consume: false,
            dispatch: crate::event::class_dispatch_for::<A>,
        });
        let index = self.class_bindings.len() - 1;
        ClassBindingBuilder {
            builder: self,
            index,
        }
    }

    /// Reports everything wrong with the bindings declared so far.
    ///
    /// [`add_context`](crate::context::ActionMapAppExt::add_context) runs this for you and refuses
    /// a context with an [`Error`](crate::plan::Severity::Error) in it, so you rarely need to call
    /// it. Where it earns its place is checking bindings you have not installed — a set read from a
    /// file, or one a player is part way through choosing — since it reads the bindings and nothing
    /// else, and needs no `App`.
    pub fn diagnostics(&self) -> Vec<crate::plan::BindingDiagnostic> {
        let mut found = crate::plan::diagnose(&self.bindings);
        found.extend(crate::plan::diagnose_classes(&self.class_bindings));
        found.extend(crate::plan::diagnose_delegated(
            &self.bindings,
            &self.delegated,
        ));
        found
    }

    /// The presentation view of these bindings: one mapping per mappable part.
    pub(crate) fn mappings(&self, context: &'static str) -> Vec<crate::mapping::ActionMapping> {
        mappings_of(&self.bindings, context)
    }

    /// The presentation view of these bindings: one row per declared tunable.
    pub(crate) fn tunables(&self, context: &'static str) -> Vec<crate::mapping::Tunable> {
        tunables_of(&self.bindings, context)
    }

    /// The controls this context withholds from capture.
    ///
    /// Flat rather than per-context, because reserving is global across a family: a screen key
    /// reserved in one context must be refused while capturing for a mapping declared in another.
    pub(crate) fn reserved(&self, context: &'static str) -> Vec<crate::capture::ReservedControl> {
        let mut reserved = Vec::new();
        for binding in self.bindings.iter().filter(|binding| binding.reserved) {
            binding.input.for_each_control(|control| {
                reserved.push(crate::capture::ReservedControl {
                    control,
                    action_path: binding.path,
                    context,
                });
            });
            // A chord's modifier keys are not reserved by reserving the chord. `Ctrl+F1` reserves
            // `F1`, and reserving `Ctrl` as well would take a modifier out of circulation for
            // every other binding in the game on the strength of one chord mentioning it.
        }
        reserved
    }

    pub(crate) fn finish(self) -> (Vec<BindingSpec>, Vec<ClassBindingSpec>, Vec<DelegatedSpec>) {
        (self.bindings, self.class_bindings, self.delegated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::action::ChannelShape;
    use crate::binding::*;
    #[cfg(feature = "gamepad")]
    use bevy_input::gamepad::GamepadButton;
    #[cfg(feature = "keyboard")]
    use bevy_input::keyboard::KeyCode;
    use bevy_math::Vec2;

    #[derive(Clone, Copy)]
    struct DummyButton;

    impl InputAction for DummyButton {
        type Output = bool;

        const INTENT: crate::action::ActionIntent = crate::action::ActionIntent::Button;
        const PATH: &'static str = "tests::DummyButton";
    }

    struct DummyVec2;

    impl InputAction for DummyVec2 {
        type Output = Vec2;

        const INTENT: crate::action::ActionIntent = crate::action::ActionIntent::Directional2;
        const PATH: &'static str = "tests::DummyVec2";
    }

    struct DummyDelta2;

    impl InputAction for DummyDelta2 {
        type Output = Vec2;

        const INTENT: crate::action::ActionIntent = crate::action::ActionIntent::Delta2;
        const PATH: &'static str = "tests::DummyDelta2";
    }
    #[cfg(feature = "keyboard")]
    #[test]
    fn binding_builders_collect_modifiers_in_order() {
        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<DummyButton>(KeyCode::Space)
            .scale(2.0)
            .negate()
            .dead_zone(DeadZone::radial(0.1));

        let (bindings, ..) = builder.finish();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].modifiers.len(), 3);
        assert!(matches!(
            bindings[0].modifiers[0],
            BindingModifier::Scale(2.0)
        ));
        assert!(matches!(bindings[0].modifiers[1], BindingModifier::Negate));
        assert!(matches!(
            bindings[0].modifiers[2],
            BindingModifier::DeadZone(_)
        ));
    }

    #[cfg(feature = "keyboard")]
    #[test]
    fn hold_or_toggle_pushes_an_inactive_toggle_and_declares_a_bool_tunable() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyButton>(KeyCode::Space);
        builder.hold_or_toggle::<DummyButton>("tests.hold_or_toggle");

        let (bindings, ..) = builder.finish();
        assert!(matches!(
            bindings[0].modifiers[0],
            BindingModifier::Toggle { active: false }
        ));
        let decl = bindings[0].tunable.as_ref().expect("declared a tunable");
        assert_eq!(decl.key, "tests.hold_or_toggle");
        assert_eq!(decl.modifier_index, 0);
        assert_eq!(decl.default, crate::mapping::TunableValue::Bool(false));
    }

    /// Two bindings of one action share one key, unasked — the whole point of declaring it once on
    /// the action rather than once per binding.
    #[cfg(feature = "keyboard")]
    #[test]
    fn hold_or_toggle_shares_one_key_across_every_eligible_binding() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyButton>(KeyCode::Space);
        builder.bind::<DummyButton>(KeyCode::Enter);
        builder.hold_or_toggle::<DummyButton>("tests.hold_or_toggle");

        let (bindings, ..) = builder.finish();
        for binding in &bindings {
            let decl = binding.tunable.as_ref().expect("declared a tunable");
            assert_eq!(decl.key, "tests.hold_or_toggle");
        }
    }

    /// `Thrust` is analog, so its trigger reads as a continuous fraction rather than a plain press
    /// — toggling that would flatten it — while the key beside it has nothing but a press to give
    /// either way. `hold_or_toggle` reaches the key and leaves the trigger alone, without being
    /// told which is which.
    #[cfg(all(feature = "keyboard", feature = "gamepad"))]
    #[test]
    fn hold_or_toggle_skips_an_analog_gamepad_button_but_reaches_the_key() {
        struct Thrust;
        impl InputAction for Thrust {
            type Output = f32;
            const INTENT: crate::action::ActionIntent = crate::action::ActionIntent::Analog1;
            const PATH: &'static str = "tests::analog_thrust";
        }

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Thrust>(GamepadButton::RightTrigger2);
        builder.bind::<Thrust>(KeyCode::KeyW);
        builder.hold_or_toggle::<Thrust>("tests.hold_or_toggle");

        let (bindings, ..) = builder.finish();
        assert!(bindings[0].tunable.is_none(), "the trigger is untouched");
        assert!(bindings[1].tunable.is_some(), "the key is toggled");
    }

    /// The one case a `GamepadButton` *is* eligible: the action itself only ever wants a plain
    /// press, so there is no analog fraction for a toggle to discard.
    #[cfg(feature = "gamepad")]
    #[test]
    fn hold_or_toggle_reaches_a_gamepad_button_when_the_action_wants_a_plain_press() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyButton>(GamepadButton::South);
        builder.hold_or_toggle::<DummyButton>("tests.hold_or_toggle");

        let (bindings, ..) = builder.finish();
        assert!(bindings[0].tunable.is_some());
    }

    /// An action bound only through analog inputs has nothing `hold_or_toggle` can toggle, and
    /// says so rather than silently doing nothing.
    #[cfg(feature = "gamepad")]
    #[test]
    #[should_panic(expected = "found no eligible binding")]
    fn hold_or_toggle_panics_when_nothing_is_eligible() {
        struct Thrust;
        impl InputAction for Thrust {
            type Output = f32;
            const INTENT: crate::action::ActionIntent = crate::action::ActionIntent::Analog1;
            const PATH: &'static str = "tests::analog_thrust_only";
        }

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Thrust>(GamepadButton::RightTrigger2);
        builder.hold_or_toggle::<Thrust>("tests.hold_or_toggle");
    }

    #[cfg(feature = "keyboard")]
    #[test]
    fn tunable_dead_zone_reads_its_range_from_the_declared_deadzone() {
        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<DummyVec2>(bevy_input::keyboard::KeyCode::Space)
            .dead_zone(DeadZone::radial(0.2))
            .tunable_dead_zone("tests.stick_deadzone", 0.0..=0.5);

        let (bindings, ..) = builder.finish();
        let decl = bindings[0].tunable.as_ref().expect("declared a tunable");
        assert_eq!(decl.key, "tests.stick_deadzone");
        assert_eq!(
            decl.default,
            crate::mapping::TunableValue::Range {
                value: 0.2,
                min: 0.0,
                max: 0.5,
            }
        );
    }

    #[cfg(feature = "keyboard")]
    #[test]
    #[should_panic(expected = "needs a `dead_zone` declared first")]
    fn tunable_dead_zone_without_a_deadzone_is_refused() {
        let mut builder = InputContextBuilder::<()>::default();
        builder
            .bind::<DummyButton>(KeyCode::Space)
            .tunable_dead_zone("tests.stick_deadzone", 0.0..=0.5);
    }

    #[cfg(feature = "keyboard")]
    #[test]
    #[should_panic(expected = "already has a tunable")]
    fn a_binding_may_declare_at_most_one_tunable() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyButton>(KeyCode::Space);
        builder.hold_or_toggle::<DummyButton>("tests.first");
        builder.hold_or_toggle::<DummyButton>("tests.second");
    }

    /// Two cases: a trigger that is button-shaped despite carrying a fraction, and a directional
    /// composite that is direction-shaped despite being made of buttons.
    /// A composite's parts are controls, not keys, so nothing stops them coming from two devices.
    #[cfg(all(feature = "keyboard", feature = "gamepad"))]
    #[test]
    fn composite_parts_are_not_tied_to_one_device() {
        let mixed = DirectionalButtons::new(
            KeyCode::KeyW,
            GamepadButton::DPadDown,
            KeyCode::KeyA,
            GamepadButton::DPadRight,
        );

        assert_eq!(mixed.up, ButtonControl::PhysicalKey(KeyCode::KeyW));
        assert_eq!(
            mixed.down,
            ButtonControl::GamepadButton(GamepadButton::DPadDown)
        );
        assert_eq!(
            DirectionalButtons::dpad().up,
            ButtonControl::GamepadButton(GamepadButton::DPadUp)
        );
        assert_eq!(
            DirectionalButtons::wasd().up,
            ButtonControl::PhysicalKey(KeyCode::KeyW)
        );
    }

    /// An analog action driven by a control that arrives on a button channel. Nothing about the
    /// binding is special, which is the point.
    #[cfg(feature = "gamepad")]
    #[test]
    fn a_trigger_can_drive_an_analog_action() {
        struct Thrust;

        impl InputAction for Thrust {
            type Output = f32;

            const INTENT: crate::action::ActionIntent = crate::action::ActionIntent::Analog1;
            const PATH: &'static str = "tests::Thrust";
        }

        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<Thrust>(GamepadButton::LeftTrigger2);
        let (bindings, class_bindings, _) = builder.finish();
        crate::plan::Plan::<()>::from_bindings(bindings, class_bindings);
    }

    /// Only one of the two can decide what the action does, and a context that says both has not
    /// said which. Nothing else a binding carries can contradict a delegated action, because
    /// `delegate` offers no way to declare it in the first place.
    #[cfg(feature = "keyboard")]
    #[test]
    fn binding_an_action_this_context_delegates_is_refused() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyButton>(KeyCode::Space);
        builder.delegate::<DummyButton>();

        let found = builder.diagnostics();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].kind,
            crate::plan::DiagnosticKind::BoundAndDelegated
        );
        assert_eq!(found[0].severity(), crate::plan::Severity::Error);
    }

    /// Declaring the same delegation twice is the same declaration, not two of them: an action has
    /// one state slot however many times a context names it.
    #[test]
    fn delegating_one_action_twice_says_it_once() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.delegate::<DummyButton>();
        builder.delegate::<DummyButton>();

        assert!(builder.diagnostics().is_empty());
        let (_, _, delegated) = builder.finish();
        assert_eq!(delegated.len(), 1);
    }

    #[cfg(feature = "keyboard")]
    #[test]
    fn a_lone_button_cannot_drive_a_directional_action() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyVec2>(KeyCode::Space);
        assert_mismatch(&builder, ChannelShape::Button);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn a_stick_cannot_stand_in_for_a_delta() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyDelta2>(Stick::Right);
        assert_mismatch(&builder, ChannelShape::Axis2);
    }

    #[test]
    fn mouse_motion_cannot_stand_in_for_a_direction() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyVec2>(MouseMove);
        assert_mismatch(&builder, ChannelShape::Delta2);
    }

    /// The three refusals above differ only in which control was offered, so they assert the same
    /// way: the diagnostic names the intent that was asked for and the channel that cannot serve
    /// it, and it is fatal rather than advisory.
    #[track_caller]
    fn assert_mismatch(builder: &InputContextBuilder<()>, shape: ChannelShape) {
        use crate::plan::{DiagnosticKind, Severity};

        let found = builder.diagnostics();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].severity(), Severity::Error);
        let DiagnosticKind::IntentMismatch { shape: found, .. } = found[0].kind else {
            panic!("expected an intent mismatch, got {:?}", found[0].kind);
        };
        assert_eq!(found, shape);
    }

    #[cfg(feature = "gamepad")]
    #[test]
    fn gamepad_source_values_bind_through_the_same_pipeline() {
        let mut builder = InputContextBuilder::<()>::default();
        builder.bind::<DummyButton>(GamepadButton::South);
        builder.bind::<DummyVec2>(Stick::Left);

        let (bindings, ..) = builder.finish();
        assert_eq!(bindings.len(), 2);
        assert!(matches!(
            bindings[0].input,
            BindingInput::GamepadButton(GamepadButton::South)
        ));
        assert!(matches!(
            bindings[1].input,
            BindingInput::GamepadStick(Stick::Left)
        ));
    }
}
