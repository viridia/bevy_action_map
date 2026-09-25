//! The Steam Input backend: the pad's half of every action, read from Steam and written into
//! [`AuthorityValues`].
//!
//! Steam owns the pad outright. The player binds it in Steam's own layout, and the game is handed a
//! value per action rather than button presses to map, which is exactly what an [`Authority`]
//! binding stands in for. Nothing here knows what the actions mean: [`SteamActions`] is a table of
//! names and how to read each one, and the game fills it in.

use bevy::prelude::*;
use bevy::scene::SceneList;
use bevy::ui_widgets::{Activate, Button};
use bevy_action_map::prelude::*;
use steamworks::{Client, Input};

use crate::common::widget_focus::focusable;
use crate::settings::{CHANGEABLE, FIXED, TITLE};

/// Spacewar's, borrowed. `docs/steam.md` S4 covers what that does and does not allow.
const APP_ID: u32 = 480;

/// `STEAM_INPUT_MAX_COUNT`, which `steamworks` does not re-export. The slice form asserts it has
/// room for this many.
const MAX_CONTROLLERS: usize = 16;

/// The Steam client, kept for as long as the app runs.
///
/// Non-send, so every system using it runs on the main thread. Steam expects `run_callbacks` from
/// one thread (S28), and a system left to the scheduler moves between workers from frame to frame.
struct Steam(Client);

/// One pad as Steam sees it.
///
/// Spawned and despawned as Steam reports the pad connecting and going. The handle is Steam's own,
/// with no relation to any OS device (S3), so this entity is the only identity the pad has here.
#[derive(Component)]
pub struct SteamController(pub u64);

/// Which of the manifest's actions the pad feeds, and which action set it is in.
#[derive(Resource)]
pub struct SteamActions {
    /// The action set to activate. Only one is live per pad at a time (S19), so the game switches
    /// it when a screen takes the controls away from the game underneath.
    pub set: &'static str,
    actions: Vec<SteamAction>,
    set_handles: Vec<(&'static str, u64)>,
    // The set last activated, so the frame of a switch is known.
    activated: u64,
    // What was last logged, so the log says only what changed.
    resolved: Option<usize>,
    pads: Option<usize>,
}

impl SteamActions {
    pub fn new(set: &'static str, actions: Vec<SteamAction>) -> Self {
        Self {
            set,
            actions,
            set_handles: Vec::new(),
            activated: 0,
            resolved: None,
            pads: None,
        }
    }
}

/// One action in the manifest, and how to read it into the crate's action of the same meaning.
///
/// Named by the action's own path (S13) unless given a name of its own. A Steam action may be
/// declared in only one set (S22), so an action the game needs live in two sets is two Steam
/// actions feeding one of the crate's. Only the one in the live set is active, and only an active
/// action writes.
pub struct SteamAction {
    name: &'static str,
    digital: bool,
    // Zero until the manifest has loaded, which happens some frames after init.
    handle: u64,
    read: fn(&Input, u64, u64, &mut AuthorityValues),
}

/// A digital action.
pub fn button<A: InputAction<Output = bool>>() -> SteamAction {
    button_as::<A>(A::PATH)
}

/// A digital action under a Steam name other than the action's path.
pub fn button_as<A: InputAction<Output = bool>>(name: &'static str) -> SteamAction {
    SteamAction {
        name,
        digital: true,
        handle: 0,
        read: |input, pad, action, values| {
            let data = input.get_digital_action_data(pad, action);
            // Packed (S11): copy the fields out rather than borrowing them.
            let (active, pressed) = (data.bActive, data.bState);
            if active {
                values.set::<A>(pressed);
            }
        },
    }
}

/// An analog action read on one axis: a trigger, or one direction of a stick.
pub fn axis<A: InputAction<Output = f32>>() -> SteamAction {
    SteamAction {
        name: A::PATH,
        digital: false,
        handle: 0,
        read: |input, pad, action, values| {
            let data = input.get_analog_action_data(pad, action);
            let (active, x) = (data.bActive, data.x);
            if active {
                values.set::<A>(x);
            }
        },
    }
}

/// An analog action read on both axes.
pub fn stick<A: InputAction<Output = Vec2>>() -> SteamAction {
    SteamAction {
        name: A::PATH,
        digital: false,
        handle: 0,
        read: |input, pad, action, values| {
            let data = input.get_analog_action_data(pad, action);
            let (active, x, y) = (data.bActive, data.x, data.y);
            if active {
                values.set::<A>(Vec2::new(x, y));
            }
        },
    }
}

/// Connects to Steam, or leaves the pad dead and the keyboard working if there is no client.
pub fn plugin(app: &mut App) {
    // Ahead of the client, since the button has to say when there is none.
    app.add_systems(
        Update,
        caption.run_if(any_with_component::<BindingPanelButton>),
    );

    let client = match Client::init_app(APP_ID) {
        Ok(client) => client,
        Err(error) => {
            warn!("Steam is unavailable ({error}); the pad will not respond");
            return;
        }
    };
    // Asked to call `run_frame` ourselves, so the read happens as late as possible before
    // evaluation rather than whenever callbacks are pumped.
    if !client.input().init(true) {
        warn!("Steam Input did not initialize; the pad will not respond");
        return;
    }
    app.insert_non_send(Steam(client));
    app.add_systems(PreUpdate, poll.before(ActionMapSystems::Evaluate));
}

/// The button under the pad's table on the controls screen, which opens Steam's binding panel.
///
/// The pad's rows are Steam's to change, so the screen lists them and this is the way to change
/// them. Its caption says when Steam cannot open the panel, and a press is ignored while that
/// holds.
pub fn binding_panel() -> Box<dyn SceneList> {
    Box::new(bsn_list! {
        BindingPanelButton
        Button
        on(open_binding_panel)
        @focusable()
        Text::new("")
        TextFont { font_size: 14.0_f32 }
        TextColor(TITLE)
        BorderColor::all(FIXED)
        Node {
            align_self: AlignSelf::Start,
            border: {UiRect::all(Val::Px(1.0))},
            border_radius: {BorderRadius::all(Val::Px(4.0))},
            padding: {UiRect::axes(Val::Px(12.0), Val::Px(3.0))},
        }
    })
}

#[derive(Component, Default, Clone, Copy)]
struct BindingPanelButton;

/// Opens the panel for the pad that flies the ship.
///
/// `NonSend` in an observer relies on commands being applied on the main thread, which the
/// executors do but do not document. Were that to change, this panics rather than misbehaves.
fn open_binding_panel(_: On<Activate>, steam: Option<NonSend<Steam>>) {
    let Some(steam) = steam else {
        return;
    };
    let input = steam.0.input();
    if let Some(&pad) = connected(&input).first()
        && !input.show_binding_panel(pad)
    {
        warn!("Steam did not open its binding panel");
    }
}

/// Says whether a press will open the panel, and why not.
fn caption(
    steam: Option<NonSend<Steam>>,
    controllers: Query<(), With<SteamController>>,
    mut buttons: Query<(&mut Text, &mut BorderColor), With<BindingPanelButton>>,
) {
    let (caption, border) = if steam.is_none() {
        ("Steam is not available", FIXED)
    } else if controllers.is_empty() {
        ("Steam sees no pad", FIXED)
    } else {
        ("Change in Steam", CHANGEABLE)
    };
    for (mut text, mut color) in &mut buttons {
        if text.0 != caption {
            text.0 = caption.into();
        }
        color.set_if_neq(BorderColor::all(border));
    }
}

/// Reads the pad and hands every context the result.
///
/// Every context entity gets the same values. A context ignores an action it does not bind, and
/// which one is live is already the mapper's business.
fn poll(
    steam: NonSend<Steam>,
    mut table: ResMut<SteamActions>,
    controllers: Query<(Entity, &SteamController)>,
    mut contexts: Query<&mut AuthorityValues>,
    mut commands: Commands,
) {
    steam.0.run_callbacks();
    let input = steam.0.input();
    input.run_frame();

    let table = &mut *table;
    for action in table.actions.iter_mut().filter(|action| action.handle == 0) {
        action.handle = if action.digital {
            input.get_digital_action_handle(action.name)
        } else {
            input.get_analog_action_handle(action.name)
        };
    }
    let resolved = table.actions.iter().filter(|a| a.handle != 0).count();
    if table.resolved != Some(resolved) {
        table.resolved = Some(resolved);
        info!(
            "Steam resolved {resolved} of {} actions",
            table.actions.len()
        );
    }
    let set = match table
        .set_handles
        .iter()
        .find(|(name, _)| *name == table.set)
    {
        Some(&(_, handle)) => handle,
        None => {
            let handle = input.get_action_set_handle(table.set);
            if handle != 0 {
                table.set_handles.push((table.set, handle));
            }
            handle
        }
    };

    let pads = connected(&input);
    if table.pads != Some(pads.len()) {
        table.pads = Some(pads.len());
        info!("Steam reports {} pads", pads.len());
    }
    for (entity, controller) in &controllers {
        if !pads.contains(&controller.0) {
            info!("Steam pad {:#x} disconnected", controller.0);
            commands.entity(entity).despawn();
        }
    }
    for &pad in &pads {
        if !controllers
            .iter()
            .any(|(_, controller)| controller.0 == pad)
        {
            info!("Steam pad {pad:#x} connected");
            commands.spawn(SteamController(pad));
        }
    }

    // One player, so the first pad Steam lists flies the ship. Starts empty, so an action nothing
    // active writes, and every action with no pad or before the manifest has loaded, reads at rest.
    let mut values = AuthorityValues::new();
    if let Some(&pad) = pads.first()
        && set != 0
    {
        // Cheap, and Valve's advice is to call it every frame rather than track what is active.
        input.activate_action_set_handle(pad, set);
        // A switch changes what every control means, so this frame supplies nothing: its data is
        // still the old set's (S24), and on the next every action the player is already holding
        // arrives unsupplied-then-held, which the mapper holds over until it is released.
        let switching = core::mem::replace(&mut table.activated, set) != set;
        for action in table
            .actions
            .iter()
            .filter(|action| action.handle != 0 && !switching)
        {
            (action.read)(&input, pad, action.handle, &mut values);
        }
    }
    for mut context in &mut contexts {
        context.clone_from(&values);
    }
}

/// The connected pads. `get_connected_controllers` always returns sixteen handles in 0.13.1, the
/// tail zeroed (S12), so this goes through the slice form and truncates to the real count.
fn connected(input: &Input) -> Vec<u64> {
    let mut handles = vec![0; MAX_CONTROLLERS];
    let count = input.get_connected_controllers_slice(&mut handles);
    handles.truncate(count);
    handles
}
