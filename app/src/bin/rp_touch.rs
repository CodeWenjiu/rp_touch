#![no_std]
#![no_main]

use defmt::{error, info};
use embassy_executor::Spawner;
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

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut class = usb_serial::init(spawner, p.USB, usb_serial::UsbSerialConfig::default());

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
                error!("USB read error: {:?}", e);
            }
        }
    }
}
