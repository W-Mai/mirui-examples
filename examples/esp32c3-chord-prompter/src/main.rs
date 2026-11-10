#![no_std]
#![no_main]

use core::cell::Cell;
use embassy_executor::Spawner;
use esp_hal::{interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup};
use esp_radio::ble::controller::BleConnector;
use mirui::gallery::demos::chord_prompter::{DeviceLink, SessionHandle};
use trouble_host::prelude::*;

mod display;
mod service;
use service::Server;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    let p = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: 200 * 1024);
    display::run(p).await;
}

async fn run_ble(
    bt: esp_hal::peripherals::BT<'static>,
    timer: esp_hal::peripherals::TIMG0<'static>,
    interrupt: esp_hal::peripherals::SW_INTERRUPT<'static>,
    session: SessionHandle,
    prepare: &Cell<bool>,
    draw: impl core::future::Future<Output = ()>,
) {
    let timg = TimerGroup::new(timer);
    let software = SoftwareInterruptControl::new(interrupt);
    esp_rtos::start(timg.timer0, software.software_interrupt0);
    let connector = BleConnector::new(bt, Default::default()).unwrap();
    let controller: ExternalController<_, 10> = ExternalController::new(connector);
    let mut resources: HostResources<DefaultPacketPool, 1, 2> = HostResources::new();
    let resource_bytes = core::mem::size_of_val(&resources);
    let rng = esp_hal::rng::Rng::new();
    let seed = rng.random();
    let mut address = [0u8; 6];
    address[..4].copy_from_slice(&seed.to_le_bytes());
    address[4..].copy_from_slice(&(rng.random() as u16).to_le_bytes());
    address[5] |= 0xc0;
    let stack =
        trouble_host::new(controller, &mut resources).set_random_address(Address::random(address));
    let Host {
        mut peripheral,
        mut runner,
        ..
    } = stack.build();
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "MIRUI Chord",
        appearance: &appearance::UNKNOWN,
    }))
    .unwrap();
    let mut link = DeviceLink::new(seed);
    server
        .set(&server.prompter.control, &link.receipt())
        .unwrap();
    esp_println::println!(
        "[BLE] server={} resources={} free={}",
        core::mem::size_of_val(&server),
        resource_bytes,
        esp_alloc::HEAP.free()
    );
    let mut ad = [0u8; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::CompleteLocalName(b"MIRUI Chord"),
        ],
        &mut ad,
    )
    .unwrap();
    embassy_futures::join::join3(
        async {
            runner.run().await.expect("BLE host runner");
        },
        draw,
        async {
            loop {
                let advertiser = peripheral
                    .advertise(
                        &Default::default(),
                        Advertisement::ConnectableScannableUndirected {
                            adv_data: &ad[..len],
                            scan_data: &[],
                        },
                    )
                    .await
                    .unwrap();
                let connection = advertiser
                    .accept()
                    .await
                    .unwrap()
                    .with_attribute_server(&server)
                    .unwrap();
                link.reconnect();
                session.set_connected(true);
                esp_println::println!("[BLE] connected");
                loop {
                    match connection.next().await {
                        GattConnectionEvent::Disconnected { .. } => {
                            link.reconnect();
                            session.set_connected(false);
                            esp_println::println!("[BLE] disconnected");
                            break;
                        }
                        GattConnectionEvent::Gatt {
                            event: GattEvent::Read(event),
                        } => {
                            if event.handle() == server.prompter.status.handle {
                                server
                                    .set(
                                        &server.prompter.status,
                                        &link.status(&session, display::clock() / 1000),
                                    )
                                    .unwrap();
                            } else if event.handle() == server.prompter.control.handle {
                                server
                                    .set(&server.prompter.control, &link.receipt())
                                    .unwrap();
                            }
                            event.accept().unwrap().send().await;
                        }
                        GattConnectionEvent::Gatt {
                            event: GattEvent::Write(event),
                        } => {
                            if let Ok(packet) = <[u8; 20]>::try_from(event.data()) {
                                let reply = link.write(&session, packet, display::clock() / 1000);
                                server.set(&server.prompter.control, &reply).unwrap();
                                if packet[0] == 0x12 && reply[0] == 0 {
                                    prepare.set(true);
                                }
                                if packet[0] != 0x11 {
                                    esp_println::println!(
                                        "[CONTROL] op={} result={} free={}",
                                        packet[0],
                                        reply[0],
                                        esp_alloc::HEAP.free()
                                    );
                                    log_memory();
                                }
                                event.accept().unwrap().send().await;
                            } else {
                                event
                                    .reject(AttErrorCode::INVALID_ATTRIBUTE_VALUE_LENGTH)
                                    .unwrap()
                                    .send()
                                    .await;
                            }
                        }
                        GattConnectionEvent::Gatt { event } => {
                            if let Ok(reply) = event.accept() {
                                reply.send().await;
                            }
                        }
                        _ => {}
                    }
                }
            }
        },
    )
    .await;
    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    esp_println::println!("[PANIC] {info}");
    loop {
        core::hint::spin_loop();
    }
}

fn log_memory() {
    #[cfg(feature = "heap-stats")]
    {
        let stats = esp_alloc::HEAP.stats();
        esp_println::println!(
            "[MEMORY] used={} peak={}",
            stats.current_usage,
            stats.max_usage
        );
    }
}
