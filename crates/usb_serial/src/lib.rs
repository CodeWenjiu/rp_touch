#![no_std]

use core::fmt;

use embassy_executor::Spawner;
use embassy_rp::{
    Peri, bind_interrupts,
    peripherals::USB,
    usb::{Driver, InterruptHandler},
};
use embassy_usb::{
    Builder,
    class::cdc_acm::{CdcAcmClass, State},
    driver::EndpointError,
};
use heapless::String;
use static_cell::StaticCell;

pub type UsbSerialClass = CdcAcmClass<'static, Driver<'static, USB>>;
const DEFAULT_WRITE_PACKET_SIZE: usize = 64;

pub struct UsbSerialConfig {
    pub vendor_id: u16,
    pub product_id: u16,
    pub manufacturer: Option<&'static str>,
    pub product: Option<&'static str>,
    pub serial_number: Option<&'static str>,
    pub max_power: u16,
    pub max_packet_size_0: u8,
    pub cdc_max_packet_size: u16,
}

impl Default for UsbSerialConfig {
    fn default() -> Self {
        Self {
            vendor_id: 0xc0de,
            product_id: 0xcafe,
            manufacturer: Some("My Custom OS"),
            product: Some("Pico 2 CDC-ACM"),
            serial_number: Some("12345678"),
            max_power: 100,
            max_packet_size_0: 64,
            cdc_max_packet_size: 64,
        }
    }
}

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

#[embassy_executor::task]
async fn usb_task(mut usb: embassy_usb::UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

pub fn init(spawner: Spawner, usb: Peri<'static, USB>, config: UsbSerialConfig) -> UsbSerialClass {
    let driver = Driver::new(usb, Irqs);
    let mut usb_config = embassy_usb::Config::new(config.vendor_id, config.product_id);
    usb_config.manufacturer = config.manufacturer;
    usb_config.product = config.product;
    usb_config.serial_number = config.serial_number;
    usb_config.max_power = config.max_power;
    usb_config.max_packet_size_0 = config.max_packet_size_0;

    static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        usb_config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [], // no msos descriptors
        CONTROL_BUF.init([0; 64]),
    );

    static STATE: StaticCell<State> = StaticCell::new();
    let state = STATE.init(State::new());
    let class = CdcAcmClass::new(&mut builder, state, config.cdc_max_packet_size);

    let usb = builder.build();
    let usb_task = usb_task(usb).unwrap();
    spawner.spawn(usb_task);
    class
}

#[derive(Debug)]
pub enum UsbSerialWriteError {
    Endpoint(EndpointError),
    BufferOverflow,
}

pub struct UsbTextWriter<'a, const BUF: usize = 128> {
    class: &'a mut UsbSerialClass,
    packet_size: usize,
}

impl<'a, const BUF: usize> UsbTextWriter<'a, BUF> {
    pub fn new(class: &'a mut UsbSerialClass) -> Self {
        Self {
            class,
            packet_size: DEFAULT_WRITE_PACKET_SIZE,
        }
    }

    pub fn with_packet_size(class: &'a mut UsbSerialClass, packet_size: usize) -> Self {
        Self {
            class,
            packet_size: packet_size.max(1),
        }
    }

    pub async fn write_str(&mut self, text: &str) -> Result<(), UsbSerialWriteError> {
        self.write_bytes(text.as_bytes()).await
    }

    pub async fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), UsbSerialWriteError> {
        let mut text = String::<BUF>::new();
        fmt::write(&mut text, args).map_err(|_| UsbSerialWriteError::BufferOverflow)?;
        self.write_str(text.as_str()).await
    }

    pub async fn writeln_fmt(
        &mut self,
        args: fmt::Arguments<'_>,
    ) -> Result<(), UsbSerialWriteError> {
        let mut text = String::<BUF>::new();
        fmt::write(&mut text, args).map_err(|_| UsbSerialWriteError::BufferOverflow)?;
        text.push_str("\r\n")
            .map_err(|_| UsbSerialWriteError::BufferOverflow)?;
        self.write_str(text.as_str()).await
    }

    async fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), UsbSerialWriteError> {
        for chunk in bytes.chunks(self.packet_size) {
            self.class
                .write_packet(chunk)
                .await
                .map_err(UsbSerialWriteError::Endpoint)?;
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! usb_print {
    ($writer:expr, $($arg:tt)*) => {{
        $writer.write_fmt(core::format_args!($($arg)*)).await
    }};
}

#[macro_export]
macro_rules! usb_println {
    ($writer:expr) => {{
        $writer.write_str("\r\n").await
    }};
    ($writer:expr, $($arg:tt)*) => {{
        $writer.writeln_fmt(core::format_args!($($arg)*)).await
    }};
}
