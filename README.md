# PadProxy

PadProxy is a Linux-first gamepad remapper written in Rust with a Qt/QML
interface through CXX-Qt. It reads a physical controller, creates a virtual
controller, and can launch a program while grabbing the source device so the
target program receives the virtual controller instead of the physical one.

The initial target is emulator launches from Lunchbox, but the project is
structured as a standalone tool with a Qt interface and YAML profiles.

The long-term reWASD-style feature target is tracked in
[`docs/REWASD_FEATURE_MATRIX.md`](docs/REWASD_FEATURE_MATRIX.md).

## Layout

- `crates/padproxy-core`: Qt-free controller discovery, profile loading, and
  remapping logic.
- `crates/padproxy-cli`: the `padproxyctl` command-line tool. This links only
  `padproxy-core`.
- `crates/padproxy-gui`: the `padproxy` Qt/QML application built with CXX-Qt.

## Current Scope

- Linux evdev controller, keyboard, and mouse discovery.
- YAML profile loading from profile directories.
- Qt/QML controller/profile browser powered by a Rust `QObject`.
- `padproxyctl launch` for running a command through a remapping profile.
- Virtual Xbox-style gamepad output through `/dev/uinput`.
- Source-device grabbing with `EVIOCGRAB` while the launched command runs.

## Requirements

On Linux, the user running PadProxy needs read access to the selected
`/dev/input/event*` controller and write access to `/dev/uinput`.

On NixOS this normally means enabling uinput and adding the user to appropriate
input/uinput groups or ACLs.

## Development

```sh
nix develop
cargo build
```

Or in one shot:

```sh
nix develop --command cargo build
```

Run the Qt interface:

```sh
nix develop --command cargo run --bin padproxy
```

List devices, outputs, profiles, and controller batteries:

```sh
nix develop --command cargo run --bin padproxyctl -- list-devices
nix develop --command cargo run --bin padproxyctl -- list-outputs
nix develop --command cargo run --bin padproxyctl -- list-profiles
nix develop --command cargo run --bin padproxyctl -- list-batteries
```

`list-batteries` reports per-device battery capacity, charge status, and
present/online state for wireless controllers and other peripherals that expose
a Linux `power_supply` node.

Detect which profile matches a running game/app:

```sh
nix develop --command cargo run --bin padproxyctl -- detect
```

`detect` scans running processes from `/proc` and prints the first profile whose
`process:` patterns match. A profile opts in by listing process names or globs:

```yaml
id: retroarch-snes
process:
  - retroarch
  - "*retroarch*"
```

Patterns match a process by basename (case-insensitively) or, when they contain
`*`/`?`, by glob against the process name.

Run a foreground remap until Ctrl-C:

```sh
nix develop --command cargo run --bin padproxyctl -- remap \
  --profile nes-2button-xa \
  --controller /dev/input/event259
```

Manage four foreground remap slots per device:

```sh
nix develop --command cargo run --bin padproxyctl -- assign-slot \
  --controller /dev/input/event259 \
  --slot 1 \
  --profile nes-2button-xa
nix develop --command cargo run --bin padproxyctl -- select-slot \
  --controller /dev/input/event259 \
  --slot 1
nix develop --command cargo run --bin padproxyctl -- apply-slot \
  --controller /dev/input/event259
nix develop --command cargo run --bin padproxyctl -- list-slots \
  --controller /dev/input/event259
```

Slots are stored in `~/.config/padproxy/slots.json`, or in
`$PADPROXY_SLOT_FILE` when that variable is set.

Export a JSON diagnostic snapshot:

```sh
nix develop --command cargo run --bin padproxyctl -- diagnostics \
  --output padproxy-diagnostics.json
```

Launch a command through a profile:

```sh
nix develop --command cargo run --bin padproxyctl -- launch \
  --profile nes-2button-xa \
  --controller /dev/input/event259 \
  -- retroarch -L /path/to/core.so /path/to/game.nes
```

## Profiles

Profiles are loaded from:

- `$PADPROXY_PROFILE_DIR`, if set. This can contain one or more platform path
  entries.
- `./profiles`, when `$PADPROXY_PROFILE_DIR` is not set
- `~/.config/padproxy/profiles.d`
- `/etc/padproxy/profiles.d`

The Qt UI can create and edit profiles from the Profiles pane. `New` starts a
structured profile, the `Mappings` tab can edit rows through dropdowns or the
Xbox/PlayStation/generic controller templates, and `Listen` can fill the active
source or target control from the selected input device. The layer
selector edits the main layer plus up to ten shift layers, each activated by a
hold or toggle control. Layers can be renamed, copied, pasted, cleared, or
removed from the structured editor. Individual mapping rows can map, disable,
or turbo a button output, keyboard key, or mouse button. Keyboard and mouse
devices are selectable as sources, so key and mouse-button rows can map back to
virtual controller buttons. Analog axes can map directly to relative mouse axes
for stick-to-mouse output. Button mappings
can fire on press, release, long press, double press, or triple press. Non-press activators
fire discrete map taps, press macros, or commands; YAML can customize
long/multi-press timing with `delay_ms`, `timeout_ms`, or `interval_ms`. The
analog panel tunes virtual axes with deadzone, sensitivity, inversion, and
output range controls, plus linear, soft, aggressive, and custom exponent
response curves. Analog zone rows can hold a virtual button, keyboard key, or
mouse button while an axis is inside a low, medium, high, or custom range.
Relative mouse motion sources can map to virtual stick axes, with automatic
recentering when motion stops.
Stick rotation rows rotate left or right stick output by degrees, and the
swap option exchanges left and right virtual stick output.
Digital stick rows convert a stick pair into the virtual D-pad with 8-way
diagonal output.
Macro rows can tap a virtual button/key/mouse button from the
structured editor or hold a virtual button until the source is released; the
editor displays the parsed macro duration beside each macro row. The
raw YAML editor also supports explicit controller/keyboard/mouse button down/up
events, relative mouse-axis events with `rel` plus `value`, axis set events,
stick deflection events with `stick` plus `x`/`y` unit values, release-side
events, pauses, and timed `cancel`/`break` events that stop queued macro
output. Command rows can run PadProxy commands such as
stopping queued and held macro output, turning the remap off with `remap_off`
(an emergency off bound to a controller button), or launch an external command
without blocking remap input. The editor lets you pick any of the supported
virtual outputs. `Apply` starts a
background remap using the selected controller and current profile contents;
`Remap Off` stops it and removes the virtual
controller. The `YAML` tab remains available for raw edits. `Save` writes the
result to
`~/.config/padproxy/profiles.d`. Packaged profiles are read-only; saving one
creates a user copy with the same profile id.

PadProxy can emulate several controller types through `/dev/uinput`: `xbox360`,
`xboxone`, `ds4` (DualShock 4), `dualsense`, and `switchpro`. Each output
advertises its own USB vendor/product identity and device name so games and
Steam Input recognize the emulated controller and apply the matching button
glyphs, while keyboard/mouse capabilities are added when the profile targets
them. Select the output per profile with the `output:` key (or in the GUI), and
list every available output with `padproxyctl list-outputs`.

Example:

```yaml
id: nes-2button-xa
name: NES 2-button X/A diamond
description: Maps a diamond-layout pad so X is run and A is jump.
match:
  name: "*"
output:
  type: xbox360
passthrough: true
grab_source: true
analog:
  swap_sticks: false
  axes:
    - code: abs:x
      deadzone: 0.15
      sensitivity: 1.25
      curve: aggressive
      invert: false
      output_min: -32768
      output_max: 32767
    - code: abs:rx
      curve: custom
      curve_exponent: 0.75
    - code: abs:rz
      zones:
        - name: high
          min: 0.66
          max: 1.00
          to: key:space
  sticks:
    - x_axis: abs:x
      y_axis: abs:y
      rotation_degrees: 15
  digital_sticks:
    - x_axis: abs:x
      y_axis: abs:y
      threshold: 0.50
mappings:
  - from: btn:south
    to: btn:south
    turbo:
      interval_ms: 75
  - from: btn:mode
    action: disable
  - from: btn:tr2
    to: key:space
  - from: btn:tl2
    to: mouse:left
  - from: abs:rx
    to: rel:x
  - from: abs:ry
    to: rel:y
  - from: rel:x
    to: abs:rx
  - from: rel:y
    to: abs:ry
  - from: btn:start
    action: macro
    macro:
      events:
        - tap: btn:south
        - pause_ms: 50
        - down: btn:east
        - pause_ms: 50
        - up: btn:east
  - from: btn:tl
    action: macro
    macro:
      mode: hold
      events:
        - down: btn:south
        - axis: abs:x
          value: 12000
      release_events:
        - axis: abs:x
          value: 0
        - up: btn:south
  - from: btn:tr
    activator:
      kind: double_press
      timeout_ms: 300
    action: macro
    macro:
      events:
        - tap: btn:north
        - stick: left
          x: 0.75
          y: -0.25
        - rel: rel:wheel
          value: -1
  - from: btn:mode
    action: macro
    macro:
      events:
        - down: btn:south
        - pause_ms: 250
        - cancel: true
  - from: btn:select
    action: command
    command: stop_macros
  - from: btn:mode
    action: command
    command:
      action: run
      shell: "notify-send PadProxy mapping-fired"
  - from: btn:east
    to: btn:west
  - from: btn:west
    to: btn:east
  - from: btn:north
    to: btn:north
```

Layered profiles can keep a flat top-level `mappings:` section for the main
layer, or use nested `layers:` entries. Shift-layer activation controls are
consumed by default so they do not leak to the virtual controller:

```yaml
id: layered-example
name: Layered example
output:
  type: xbox360
layers:
  - id: main
    name: Main
    mappings:
      - from: btn:south
        to: btn:south
  - id: shift_1
    name: Shift 1
    activation:
      mode: hold
      control: btn:tl
      consume: true
    mappings:
      - from: btn:south
        to: btn:east
```
