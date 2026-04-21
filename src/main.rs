#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    peripherals::USB,
    usb::{Driver, InterruptHandler},
};
use embassy_usb::{
    Builder,
    class::cdc_acm::{CdcAcmClass, State},
};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

// Program metadata for `picotool info`.
// This isn't needed, but it's recommended to have these minimal entries.
#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Blinky Example"),
    embassy_rp::binary_info::rp_program_description!(
        c"This example tests the RP Pico on board LED, connected to gpio 25"
    ),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

// 1. 绑定 USB 硬件中断到 Embassy 的 USB 驱动
bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let driver = Driver::new(p.USB, Irqs);
    let mut config = embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("My Custom OS");
    config.product = Some("Pico 2 CDC-ACM");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [], // no msos descriptors
        CONTROL_BUF.init([0; 64]),
    );

    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());
    let mut class = CdcAcmClass::new(&mut builder, state, 64);

    let usb = builder.build();
    spawner.spawn(unwrap!(usb_task(usb)));

    class.wait_connection().await;
    info!("PC connected via USB CDC!");

    let mut buf = [0u8; 64];
    loop {
        let hello = b"Hello from Pico 2!\r\n";
        let _ = class.write_packet(hello).await;

        match class.read_packet(&mut buf).await {
            Ok(n) => {
                info!("Received {} bytes: {:?}", n, &buf[..n]);
                let _ = class.write_packet(&buf[..n]).await;
            }
            Err(e) => {
                defmt::error!("USB read error: {:?}", e);
            }
        }
    }
}
