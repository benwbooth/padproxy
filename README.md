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

- Linux evdev controller discovery.
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

List devices, outputs, and profiles:

```sh
nix develop --command cargo run --bin padproxyctl -- list-devices
nix develop --command cargo run --bin padproxyctl -- list-outputs
nix develop --command cargo run --bin padproxyctl -- list-profiles
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
source or target control from the selected physical controller. The layer
selector edits the main layer plus up to ten shift layers, each activated by a
hold or toggle control. The editor shows implemented and planned virtual
outputs, but only implemented outputs can be applied. `Apply` starts a
background remap using the selected controller and current profile contents;
`Remap Off` stops it and removes the virtual controller. The `YAML` tab remains
available for raw edits. `Save` writes the result to
`~/.config/padproxy/profiles.d`. Packaged profiles are read-only; saving one
creates a user copy with the same profile id.

The only implemented virtual output today is `xbox360`. Other reWASD-style
virtual outputs are listed by the UI and `padproxyctl list-outputs` as planned,
but they are intentionally not selectable for Apply until the device backend
exists.

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
mappings:
  - from: btn:south
    to: btn:south
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
