//! Bluetooth controller emulation: present PadProxy's controller to another
//! host as a Bluetooth HID-over-GATT gamepad.
//!
//! This reads a source controller and serves a BLE HID peripheral whose input
//! reports mirror the controller. Pairing/connecting requires a Bluetooth
//! adapter that supports peripheral mode and a host willing to bond, so the
//! transport is exercised with real hardware; the report core is unit-tested in
//! `padproxy_core::bthid`.

use anyhow::{Context, Result};
use bluer::adv::Advertisement;
use bluer::gatt::local::{
    Application, Characteristic, CharacteristicNotify, CharacteristicNotifyMethod,
    CharacteristicRead, Descriptor, DescriptorRead, Service,
};
use bluer::Uuid;
use evdev::Device;
use padproxy_core::bthid::{encode_hid_report, HID_REPORT_DESCRIPTOR, HID_REPORT_LEN};
use padproxy_core::gimx::GamepadState;
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::sync::watch;

const APPEARANCE_GAMEPAD: u16 = 0x03C4;

fn uuid16(short: u16) -> Uuid {
    Uuid::from_u128(0x0000_0000_0000_1000_8000_00805f9b34fb_u128 | ((short as u128) << 96))
}

/// Entry point: build a Tokio runtime and run the BLE HID peripheral.
pub fn run(controller_path: &str) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start async runtime")?;
    runtime.block_on(serve(controller_path.to_string()))
}

async fn serve(controller_path: String) -> Result<()> {
    // Latest HID input report, updated by the controller reader thread.
    let (report_tx, report_rx) = watch::channel([0u8; HID_REPORT_LEN]);

    // Read the controller on a blocking thread and publish encoded reports.
    let reader_path = controller_path.clone();
    std::thread::spawn(move || {
        if let Err(error) = read_controller(&reader_path, report_tx) {
            eprintln!("PadProxy controller reader stopped: {error}");
        }
    });

    let session = bluer::Session::new()
        .await
        .context("failed to connect to BlueZ")?;
    let adapter = session
        .default_adapter()
        .await
        .context("no Bluetooth adapter")?;
    adapter.set_powered(true).await?;

    let report_map = HID_REPORT_DESCRIPTOR.to_vec();
    let notify_rx = report_rx.clone();

    let app = Application {
        services: vec![Service {
            uuid: uuid16(0x1812), // Human Interface Device
            primary: true,
            characteristics: vec![
                // Report Map: the HID descriptor.
                Characteristic {
                    uuid: uuid16(0x2A4B),
                    read: Some(CharacteristicRead {
                        read: true,
                        fun: Box::new(move |_| {
                            let value = report_map.clone();
                            Box::pin(async move { Ok(value) })
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                // HID Information: bcdHID 1.11, country 0, flags (remote wake).
                Characteristic {
                    uuid: uuid16(0x2A4A),
                    read: Some(CharacteristicRead {
                        read: true,
                        fun: Box::new(|_| Box::pin(async { Ok(vec![0x11, 0x01, 0x00, 0x02]) })),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                // Input Report: notify with the latest report; Report Reference
                // descriptor marks it as input report id 0.
                Characteristic {
                    uuid: uuid16(0x2A4D),
                    read: Some(CharacteristicRead {
                        read: true,
                        fun: {
                            let rx = report_rx.clone();
                            Box::new(move |_| {
                                let value = rx.borrow().to_vec();
                                Box::pin(async move { Ok(value) })
                            })
                        },
                        ..Default::default()
                    }),
                    notify: Some(CharacteristicNotify {
                        notify: true,
                        method: CharacteristicNotifyMethod::Fun(Box::new(move |mut notifier| {
                            let mut rx = notify_rx.clone();
                            Box::pin(async move {
                                while rx.changed().await.is_ok() {
                                    let report = rx.borrow().to_vec();
                                    if notifier.notify(report).await.is_err() {
                                        break;
                                    }
                                }
                            })
                        })),
                        ..Default::default()
                    }),
                    descriptors: vec![Descriptor {
                        uuid: uuid16(0x2908), // Report Reference
                        read: Some(DescriptorRead {
                            read: true,
                            fun: Box::new(|_| Box::pin(async { Ok(vec![0x00, 0x01]) })),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let _app_handle = adapter
        .serve_gatt_application(app)
        .await
        .context("failed to serve the HID GATT application")?;

    let advertisement = Advertisement {
        service_uuids: vec![uuid16(0x1812)].into_iter().collect::<BTreeSet<_>>(),
        appearance: Some(APPEARANCE_GAMEPAD),
        discoverable: Some(true),
        local_name: Some("PadProxy Gamepad".to_string()),
        ..Default::default()
    };
    let _adv_handle = adapter
        .advertise(advertisement)
        .await
        .context("failed to start advertising")?;

    eprintln!(
        "PadProxy is advertising a Bluetooth HID gamepad ({}). Pair from the target host. Press Ctrl-C to stop.",
        adapter.name()
    );

    // Keep the handles alive until interrupted.
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

fn read_controller(path: &str, report_tx: watch::Sender<[u8; HID_REPORT_LEN]>) -> Result<()> {
    let mut source =
        Device::open(path).with_context(|| format!("failed to open controller {path}"))?;
    let mut state = GamepadState::default();
    loop {
        for event in source
            .fetch_events()
            .context("failed reading controller events")?
        {
            if state.apply_event(&event) {
                let _ = report_tx.send(encode_hid_report(&state));
            }
        }
    }
}
