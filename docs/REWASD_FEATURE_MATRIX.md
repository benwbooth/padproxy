# reWASD Feature Matrix for PadProxy

This is the target feature inventory for making PadProxy feel closer to
reWASD. It is not a promise that every item lands at once; it is the backlog we
can implement against.

Source pages consulted:

- https://www.rewasd.com/
- https://www.rewasd.com/advanced-controller-mapping
- https://help.rewasd.com/installation-notes/basic-and-advanced-features.html
- https://help.rewasd.com/interface/workspace.html
- https://help.rewasd.com/how-to-remap/controller.html
- https://help.rewasd.com/how-to-remap/supported-devices.html
- https://help.rewasd.com/types-of-mappings/controller-mapping.html
- https://help.rewasd.com/types-of-mappings/gamepad-mapping.html
- https://help.rewasd.com/types-of-mappings/commands.html
- https://help.rewasd.com/basic-functions/shift-mode.html
- https://help.rewasd.com/basic-functions/advanced-stick-settings.html
- https://help.rewasd.com/basic-functions/sticks-and-trigger-zones.html
- https://help.rewasd.com/mapping-features/key-combo.html
- https://help.rewasd.com/mapping-features/mobile-controller.html
- https://help.rewasd.com/basic-functions/miscellaneous-features.html
- https://help.rewasd.com/interface/command-line.html
- https://help.rewasd.com/external-devices/bluetooth.html

## Core UX

- Profiles: game/app-level containers for configurations.
- Configs: individual mapping layouts inside a profile.
- Sub-configs: per-device layouts when several devices are grouped.
- Device panel: connected, disconnected, hidden, grouped, physical, and virtual
  devices.
- Controller image editor: clickable gamepad controls for selecting mappings.
- Hook mode: press a physical controller, keyboard, or mouse control to select
  it in the editor.
- Apply vs Save: save a config or apply it immediately to a running remap.
- Remap ON/OFF status.
- Community preset browser/import/export. Basic local import/export done:
  `padproxyctl export-profile` writes a profile's YAML preset and
  `padproxyctl import-profile` validates and installs a preset into the user
  profile directory. The online community browser remains.
- Background tray/daemon. Basic background daemon done: `padproxyctl serve`
  runs a long-lived control service over a local socket; a system-tray icon
  remains.

## Mapping Types

- Controller to controller mappings. Basic button/axis remap done.
- Controller to keyboard mappings. Basic controller-button to keyboard-key
  mappings done.
- Controller to mouse mappings. Basic controller-button to mouse-button
  mappings done; relative mouse-axis output is available in YAML macros and
  direct analog-axis mappings.
- Keyboard to controller mappings. Basic keyboard-device discovery and
  keyboard-key to virtual-controller mappings done.
- Mouse to controller mappings. Basic mouse-button to controller-button
  mappings and relative mouse-motion to centered virtual-stick mappings done.
- Native/hardware mappings that do not create an emulated device.
- Virtual mappings that emit through a virtual device.
- Mute physical input while virtual output is active. Done: `grab_source`
  grabs the physical device with `EVIOCGRAB` so its input is muted from other
  apps while the virtual controller drives output.
- Disable a source button entirely. Done for controller inputs.
- Commands, such as turn remap off. Basic PadProxy command mappings done for
  stopping queued and held macros, running external commands, and a `remap_off`
  command that turns the active remap off from a bound button.
- Launch app / run command mappings. Basic non-blocking external command
  launch done for controller command mappings.

## Activators

- Single press. Done for controller mappings.
- Long press. Basic controller-button activator done.
- Double press. Basic controller-button activator done.
- Triple press. Basic controller-button activator done.
- Start press. Done as the default press activator for controller mappings.
- Release press. Basic controller-button activator done.
- Hold until release. Basic hold macros and hold shift layers done.
- Toggle. Basic toggle shift layers done.
- Turbo / rapid fire. Done for held controller button remaps.
- Custom activator delays and timings. Basic `delay_ms`/`timeout_ms`/`interval_ms`
  YAML timing done for long, double, and triple press.

## Combos / Macros

- Keyboard, mouse, and controller macro events. Basic controller button/axis
  macro events done; keyboard and mouse-button macro taps/down/up and relative
  mouse-axis macro events are done.
- Explicit down/up events. Done for controller, keyboard, and mouse-button
  macro events.
- Pauses. Done for controller macros.
- Rumble events. Basic rumble passthrough done: when the source controller
  supports force feedback, the virtual pad advertises rumble and forwards game
  force-feedback effects (upload/play/stop/erase) to the physical controller.
- Break/cancel events. Basic YAML macro cancel/break/stop events done; they
  stop queued and held macro output at their scheduled point in the sequence.
- Execute-at-once macros. Done for press-triggered controller macros.
- Hold-until-release macros. Basic controller hold macros done.
- Relative stick deflection in macros. Basic YAML `stick` macro events done for
  left/right stick x/y unit deflection.
- Macro duration display. Basic parsed duration display done in the structured
  editor for press and hold macros.
- Stop all macros shortcut. Done as a controller command mapping.

## Layers and Slots

- Main layer plus shift layers.
- Up to 10 shift overlays.
- Shift activation by hold, toggle, custom activator, or shortcut.
- Copy, paste, rename, and clear layers. Basic structured-editor operations
  done.
- Four config slots per device. Basic persistent CLI slots done.
- Slot switching by hotkey. Done: `next_slot`, `prev_slot`, and
  `select_slot` (with a `slot:`) command mappings switch the controller's
  active slot during a `padproxyctl apply-slot` session.
- Apply, select, and clear slots from CLI/API. Done: CLI assign, select,
  apply, list, and clear slot commands, plus matching `assign_slot`,
  `select_slot`, `clear_slot`, `list_slots`, and `apply_slot` commands over the
  local control API.

## Analog Controls

- Stick deadzones. Basic per-axis deadzone done.
- Trigger zones. Basic held-output axis zones done.
- Low, medium, and high zone mappings. Basic button/key/mouse-button zone
  outputs done.
- Stick response curves. Basic linear/soft/aggressive response curves done.
- Custom response curves. Basic YAML/UI custom exponent curve done.
- Axis inversion. Basic per-axis inversion done.
- Axis range tuning. Basic per-axis output min/max done.
- Stick rotation. Basic left/right stick rotation by degrees done.
- Swap sticks. Basic left/right virtual stick swap done.
- Digital 8-way stick mode. Basic stick-pair to virtual D-pad output done.
- Diagonal stick mappings. Basic diagonal output done through digital stick
  X/Y output.
- Mouse-to-stick conversion. Basic relative mouse-axis to centered virtual
  stick-axis conversion done with automatic recentering.
- Stick-to-mouse conversion. Basic direct analog-axis to relative mouse-axis
  mappings done.
- Per-axis sensitivity. Basic per-axis sensitivity done.

## Output Devices

- Virtual Xbox 360. Done through a Linux uinput device with the Xbox 360
  USB identity.
- Virtual Xbox One / Series. Done through a Linux uinput device with the Xbox
  One USB identity.
- Virtual DualShock 4. Done through a Linux uinput device with the DualShock 4
  USB identity.
- Virtual DualSense. Done through a Linux uinput device with the DualSense USB
  identity.
- Virtual Switch Pro. Done through a Linux uinput device with the Switch Pro
  USB identity.
- Keyboard output. Basic keyboard-key capabilities are added to the Linux
  uinput virtual device when profile mappings or macros target them.
- Mouse output. Basic mouse-button and relative-axis capabilities are added to
  the Linux uinput virtual device when profile mappings or macros target them.
- Multiple virtual devices from one profile. Done: a profile `outputs:` list
  creates additional virtual gamepads that mirror the primary pad's output.
- External controller emulation over Bluetooth. Not implemented: presenting as
  a Bluetooth gamepad to another console needs a BlueZ HID-gadget/configfs stack,
  which is separate hardware/kernel infrastructure.
- GIMX-style wired external output. Not implemented: needs a GIMX USB adapter
  and its serial protocol.

## Device Support

Discovery and remapping work for any standard Linux evdev controller. The
families below are recognized by name so they appear in the device list and can
be remapped; device-specific extras (Elite paddles, gyro, touchpad, individual
Joy-Cons) are tracked separately under Sensors/Grouping.

- Xbox 360 / One / Series. Recognized and remappable.
- Xbox Elite paddles. Done: paddle buttons (`btn:paddle1`-`btn:paddle4`, the
  evdev `BTN_TRIGGER_HAPPY` codes) are recognized as source inputs and can be
  mapped like any other button.
- DualShock 3 / 4. Recognized and remappable.
- DualSense / DualSense Edge. Recognized and remappable.
- PlayStation Navigation controller. Recognized by name.
- Steam Controller. Recognized and remappable.
- Steam Deck controller. Recognized and remappable.
- Nintendo Switch Pro. Recognized and remappable.
- Nintendo Joy-Con pairs and individual Joy-Cons. Joy-Cons recognized; pairing
  two into one virtual controller uses device grouping.
- GameCube controllers. Recognized and remappable.
- Stadia controller. Recognized and remappable.
- NVIDIA Shield controller. Recognized and remappable.
- Azeron and keypad-style devices. Recognized by name.
- Generic HID gamepads. Done: any evdev gamepad with face buttons and X/Y axes
  is discovered and remappable.
- Keyboard and mouse devices with initialization/detection mode. Basic Linux
  evdev keyboard/mouse discovery done; richer initialization/detection flow
  remains.
- Mobile controller from Android/iOS. Basic mobile controller done:
  `padproxyctl mobile-server` listens on UDP for JSON controller-state packets
  (`{"buttons":{...},"axes":{...}}`) and drives a virtual gamepad, so a phone
  app can act as a controller.

## Device Grouping

- Group multiple devices into one virtual controller. Basic grouping done: a
  profile `group:` list of device matchers opens additional source devices and
  merges their input into the same virtual controller.
- Per-device sub-configs inside a group. Basic sub-configs done: each `group:`
  member can carry a `remap:` of its own input codes (applied before the shared
  profile mapping), so grouped devices have per-device layouts.
- Ordered same-type devices, such as Gamepad 1 and Gamepad 2. Basic ordering
  done: group matchers resolve to distinct devices in listed order, so two
  same-type controllers map to ordered group members.
- Joy-Con grouping.
- Phone gyro grouped with physical gamepad.

## Sensors and Haptics

- Gyroscope mappings. Basic gyro support done: motion-sensor devices are now
  discovered, so a controller's gyro device can be grouped and its rotation
  axes mapped to mouse or stick output via the analog axis-to-mouse mappings.
- Gyro activation only on shift/layer/hold. Done via layers: place the gyro
  axis mappings in a hold/toggle shift layer so they apply only while active.
- Touchpad zones. Basic touchpad zones done: a profile `touchpad:` config maps
  a touchpad device's normalized touch regions to button/key outputs, pressing
  the matching zone's output while a finger is in it.
- Controller vibration settings. Game-driven rumble is forwarded to the source
  controller (see rumble passthrough under Combos/Macros).
- Rumble mappings. Basic rumble passthrough done; explicit profile-driven rumble
  triggers remain.
- LED settings. Basic LED control done: `padproxyctl list-leds` enumerates
  Linux `sys/class/leds` devices (brightness, max, and RGB `multi_intensity`),
  and `padproxyctl set-led` sets brightness (clamped) and RGB color.
- Battery and wireless status where available. Basic device-scoped battery
  capacity, charge status, and present/online state are read from Linux sysfs
  `power_supply` nodes and exposed through `padproxyctl list-batteries` and the
  diagnostics export.
- Wireless controller power-off command. Done: `padproxyctl power-off`
  disconnects a Bluetooth controller via BlueZ (`bluetoothctl disconnect`) using
  the address from the device's `uniq`/`phys`.

## Overlays and Menus

- Desktop overlay. Basic overlay done: `padproxy --overlay desktop` shows an
  always-on-top remap on/off and active-profile HUD (live backend wiring
  pending).
- Dynamic active-layer display. Basic overlay done: `padproxy --overlay layer`
  shows an always-on-top badge with the active layer name (live backend wiring
  pending).
- Radial menu. Basic overlay done: `padproxy --overlay radial` shows a ring of
  labelled command segments around a center hub (profile-command binding
  pending).
- Crosshair overlay. Basic crosshair done: `padproxy --overlay crosshair`
  shows a transparent, always-on-top, click-through full-screen window that
  draws a center crosshair.
- Screen magnifier. Basic magnifier done: `padproxy --overlay magnifier` shows
  a zoomed view of the screen center using Qt's `ScreenCapture` (works on X11
  and on Wayland where a screencast portal is available); `+`/`-` change zoom.

## Automation and Integration

- Autodetect game process and apply profile. Done: profiles declare `process:`
  name/glob patterns, `padproxyctl detect` reports the matching profile, and
  `padproxyctl watch` applies the matched profile automatically, switching when
  the foreground game changes and stopping when it exits.
- CLI apply/select-slot/clear-slot/remap/version/help. Basic foreground
  `remap`/`apply`, `apply-slot`, `launch`, list, slot management, version, and
  help commands done.
- Local API/DBus service. Done: `padproxyctl serve` exposes a Unix-socket JSON
  API (status, list profiles/devices/batteries, detect, slot management, and
  live apply/remap-off) with `padproxyctl ctl` as a client, and `padproxyctl
  dbus` exposes the same JSON protocol on the session bus via
  `com.benwbooth.PadProxy1.Request`.
- Per-emulator/game launch integration. Done: `padproxyctl launch` grabs the
  controller, applies a profile, and runs the program; `--profile` is optional
  and auto-selected by matching the launched command against profiles'
  `process:` patterns.
- Emergency remap off. Done through a `remap_off` command mapping that stops
  the running remap, releases the virtual device, and ungrabs the source
  controller; during a launch it leaves the launched program running.
- Blocklist for apps/games where remap should not apply. Done: a
  `blocklist.txt` of process name/glob patterns (viewable with
  `padproxyctl list-blocklist`) keeps `padproxyctl watch` from applying, and
  stops any active remap, while a blocked process is running.
- Logging and diagnostic export. JSON diagnostic export is done with
  `padproxyctl diagnostics`, and structured runtime logging is done: leveled,
  timestamped log lines are emitted to stderr for remap lifecycle and command
  events, controlled by the `PADPROXY_LOG` level (`error`/`warn`/`info`/`debug`/
  `trace`/`off`, default `info`).

## Near-Term PadProxy Milestones

1. Replace YAML-first editing with a structured mapping editor. Done.
2. Add clickable SVG controller images for Xbox, PlayStation, and generic pads. Done.
3. Add hook mode to pick source and target controls by pressing buttons. Done.
4. Add remap ON/OFF/apply lifecycle from the GUI. Done.
5. Add virtual output type selection and hide/grab policy controls. Done; the Xbox 360, Xbox One, DualShock 4, DualSense, and Switch Pro virtual outputs each emulate their own USB identity through uinput and can all be applied.
6. Add layers, then activators, then macros. Basic main/shift layers are done with hold/toggle activation, disable mappings, held-button turbo, press/release/long/double/triple controller activators, press-triggered controller/keyboard/mouse-button macros, relative mouse macro events, and hold-until-release controller macros; richer activators and macros remain.
7. Add analog tuning. Basic per-axis deadzone, sensitivity, response curves, custom curve exponent, held-output zones, stick rotation, stick swap, digital 8-way stick mode, inversion, and output range are done.
8. Add process autodetect and Lunchbox integration. Process autodetection is done through profile `process:` patterns, `padproxyctl detect`, and `padproxyctl watch` (auto-apply on detect); Lunchbox launch integration remains.
