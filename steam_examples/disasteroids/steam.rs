//! The Steam Input backend: the pad's half of every action, read from Steam and written into
//! [`AuthorityValues`].
//!
//! Steam owns the pad outright. The player binds it in Steam's own layout, and the game is handed a
//! value per action rather than button presses to map, which is exactly what an [`Authority`]
//! binding stands in for. Nothing here knows what the actions mean: [`SteamActions`] is a table of
//! names and how to read each one, and the game fills it in.

use bevy::prelude::*;
use bevy_action_map::prelude::*;
use steamworks::{Client, Input};

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
    app.add_systems(Update, open_binding_panel);
}

/// TEMPORARY, removed by chunk 151f: F12 opens Steam's binding panel for the first pad, until the
/// controls screen's delegated row does it properly.
fn open_binding_panel(
    keys: Res<ButtonInput<KeyCode>>,
    steam: NonSend<Steam>,
    controllers: Query<&SteamController>,
) {
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }
    match controllers.iter().next() {
        Some(pad) => info!(
            "show_binding_panel -> {}",
            steam.0.input().show_binding_panel(pad.0)
        ),
        None => warn!("show_binding_panel: no pad"),
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
