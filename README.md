# PadProxy

PadProxy is a Linux-first gamepad remapper written in Rust with a Qt/QML
interface through CXX-Qt. It reads a physical controller, creates a virtual
controller, and can launch a program while grabbing the source device so the
target program receives the virtual controller instead of the physical one.

The initial target is emulator launches from Lunchbox, but the project is
structured as a standalone tool with a Qt interface and YAML profiles.

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

List devices and profiles:

```sh
nix develop --command cargo run --bin padproxyctl -- list-devices
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
YAML profile template, `Edit` loads the selected profile YAML, and `Save` writes
the result to `~/.config/padproxy/profiles.d`. Packaged profiles are read-only;
saving one creates a user copy with the same profile id.

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
