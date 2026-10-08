# padprobe

Prints what a gamepad reports through [gilrs](https://crates.io/crates/gilrs), the library Bevy
reads gamepads with, without building a Bevy app.

## Running

You need a Rust toolchain. On Linux, gilrs also needs the udev headers (`libudev-dev` on Debian and
Ubuntu, `systemd-devel` on Fedora). From the repository root:

```sh
cargo run --manifest-path tools/padprobe/Cargo.toml --release -- 30 --bevy
```

`30` is how many seconds it listens for. `--bevy` turns off gilrs's own filtering the way Bevy does,
so what it prints is what a Bevy game would receive.

## Testing a Nintendo pad

We want to know which button position a Nintendo pad's **A** reports as. Bevy names face buttons by
position (`South`, `East`, `North`, `West`), and a Nintendo pad's A is on the right, where an Xbox
pad's B is. The answer has only been read from SDL's controller database so far, because neither pad
we tried on macOS delivered any usable input.

1. Close Steam if it is running, since it can claim the pad.
2. Connect a Switch Pro Controller, by USB or Bluetooth. If you also have Joy-Cons, a second run
   with one of them is welcome.
3. Run the command above, and press **A**, **B**, **X** and **Y** once each, in that order, by the
   letters printed on them, with a second or so between presses.
4. Send back the whole output, along with:
   - the operating system and its version
   - which controller, and whether it was connected by USB or Bluetooth
   - on Linux, the output of `lsmod | grep nintendo`, since the kernel's own Nintendo driver changes
     what the pad looks like

The lines that answer the question look like this:

```text
[0] PRESS East   (code BUTTON(2))
```

If the pad connects but no `PRESS` lines appear, or they appear without anything being pressed,
please send that output too. It is what we saw on macOS, and knowing whether it happens elsewhere is
useful in itself.
