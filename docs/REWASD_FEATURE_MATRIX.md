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
- Community preset browser/import/export.
- Background tray/daemon.

## Mapping Types

- Controller to controller mappings. Basic button/axis remap done.
- Controller to keyboard mappings. Basic controller-button to keyboard-key
  mappings done.
- Controller to mouse mappings. Basic controller-button to mouse-button
  mappings done; relative mouse-axis output is available in YAML macros.
- Keyboard to controller mappings.
- Mouse to controller mappings.
- Native/hardware mappings that do not create an emulated device.
- Virtual mappings that emit through a virtual device.
- Mute physical input while virtual output is active.
- Disable a source button entirely. Done for controller inputs.
- Commands, such as turn remap off. Basic PadProxy command mappings done for
  stopping queued and held macros.
- Launch app / run command mappings.

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
- Rumble events.
- Break/cancel events.
- Execute-at-once macros. Done for press-triggered controller macros.
- Hold-until-release macros. Basic controller hold macros done.
- Relative stick deflection in macros.
- Macro duration display.
- Stop all macros shortcut. Done as a controller command mapping.

## Layers and Slots

- Main layer plus shift layers.
- Up to 10 shift overlays.
- Shift activation by hold, toggle, custom activator, or shortcut.
- Copy, paste, rename, and clear layers.
- Four config slots per device.
- Slot switching by hotkey.
- Apply, select, and clear slots from CLI/API.

## Analog Controls

- Stick deadzones. Basic per-axis deadzone done.
- Trigger zones.
- Low, medium, and high zone mappings.
- Stick response curves.
- Custom response curves.
- Axis inversion. Basic per-axis inversion done.
- Axis range tuning. Basic per-axis output min/max done.
- Stick rotation.
- Swap sticks.
- Digital 8-way stick mode.
- Diagonal stick mappings.
- Mouse-to-stick conversion.
- Stick-to-mouse conversion.
- Per-axis sensitivity. Basic per-axis sensitivity done.

## Output Devices

- Virtual Xbox 360.
- Virtual Xbox One / Series.
- Virtual DualShock 4.
- Virtual DualSense.
- Virtual Switch Pro.
- Keyboard output. Basic keyboard-key capabilities are added to the Linux
  uinput virtual device when profile mappings or macros target them.
- Mouse output. Basic mouse-button and relative-axis capabilities are added to
  the Linux uinput virtual device when profile mappings or macros target them.
- Multiple virtual devices from one profile.
- External controller emulation over Bluetooth.
- GIMX-style wired external output.

## Device Support

- Xbox 360 / One / Series.
- Xbox Elite paddles.
- DualShock 3 / 4.
- DualSense / DualSense Edge.
- PlayStation Navigation controller.
- Steam Controller.
- Steam Deck controller.
- Nintendo Switch Pro.
- Nintendo Joy-Con pairs and individual Joy-Cons.
- GameCube controllers.
- Stadia controller.
- NVIDIA Shield controller.
- Azeron and keypad-style devices.
- Generic HID gamepads.
- Keyboard and mouse devices with initialization/detection mode.
- Mobile controller from Android/iOS.

## Device Grouping

- Group multiple devices into one virtual controller.
- Per-device sub-configs inside a group.
- Ordered same-type devices, such as Gamepad 1 and Gamepad 2.
- Joy-Con grouping.
- Phone gyro grouped with physical gamepad.

## Sensors and Haptics

- Gyroscope mappings.
- Gyro activation only on shift/layer/hold.
- Touchpad zones.
- Controller vibration settings.
- Rumble mappings.
- LED settings.
- Battery and wireless status where available.
- Wireless controller power-off command.

## Overlays and Menus

- Desktop overlay.
- Dynamic active-layer display.
- Radial menu.
- Crosshair overlay.
- Screen magnifier.

## Automation and Integration

- Autodetect game process and apply profile.
- CLI apply/select-slot/clear-slot/remap/version/help.
- Local API/DBus service.
- Per-emulator/game launch integration.
- Emergency remap off.
- Blocklist for apps/games where remap should not apply.
- Logging and diagnostic export.

## Near-Term PadProxy Milestones

1. Replace YAML-first editing with a structured mapping editor. Done.
2. Add clickable SVG controller images for Xbox, PlayStation, and generic pads. Done.
3. Add hook mode to pick source and target controls by pressing buttons. Done.
4. Add remap ON/OFF/apply lifecycle from the GUI. Done.
5. Add virtual output type selection and hide/grab policy controls. Done for implemented outputs; planned outputs are visible but disabled until their virtual-device backends exist.
6. Add layers, then activators, then macros. Basic main/shift layers are done with hold/toggle activation, disable mappings, held-button turbo, press/release/long/double/triple controller activators, press-triggered controller/keyboard/mouse-button macros, relative mouse macro events, and hold-until-release controller macros; richer activators and macros remain.
7. Add analog tuning. Basic per-axis deadzone, sensitivity, inversion, and output range are done.
8. Add process autodetect and Lunchbox integration.
