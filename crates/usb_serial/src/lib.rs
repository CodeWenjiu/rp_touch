#![no_std]

extern crate alloc;

use alloc::string::String;
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
use static_cell::StaticCell;

pub type UsbSerialClass = CdcAcmClass<'static, Driver<'static, USB>>;
const DEFAULT_WRITE_PACKET_SIZE: usize = 64;
const DEFAULT_FORMAT_BUF_SIZE: usize = 128;

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
    AllocFailed,
}

pub struct UsbTextWriter<'a> {
    class: &'a mut UsbSerialClass,
    packet_size: usize,
    fmt_buf: String,
}

impl<'a> UsbTextWriter<'a> {
    pub fn new(class: &'a mut UsbSerialClass) -> Self {
        Self::with_packet_size(class, DEFAULT_WRITE_PACKET_SIZE)
    }

    pub fn with_packet_size(class: &'a mut UsbSerialClass, packet_size: usize) -> Self {
        Self {
            class,
            packet_size: packet_size.max(1),
            fmt_buf: String::new(),
        }
    }

    pub async fn wait_connection(&mut self) {
        self.class.wait_connection().await;
    }

    pub async fn read_packet(&mut self, data: &mut [u8]) -> Result<usize, EndpointError> {
        self.class.read_packet(data).await
    }

    pub async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        self.class.write_packet(data).await
    }

    pub async fn write_str(&mut self, text: &str) -> Result<(), UsbSerialWriteError> {
        self.write_bytes(text.as_bytes()).await
    }

    pub async fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), UsbSerialWriteError> {
        let mut fmt_buf = core::mem::take(&mut self.fmt_buf);
        reserve_at_least(&mut fmt_buf, DEFAULT_FORMAT_BUF_SIZE)
            .map_err(|_| UsbSerialWriteError::AllocFailed)?;
        fmt_buf.clear();
        let mut sink = FallibleFmtSink(&mut fmt_buf);
        fmt::write(&mut sink, args).map_err(|_| UsbSerialWriteError::AllocFailed)?;
        let res = self.write_bytes(fmt_buf.as_bytes()).await;
        self.fmt_buf = fmt_buf;
        res
    }

    pub async fn writeln_fmt(
        &mut self,
        args: fmt::Arguments<'_>,
    ) -> Result<(), UsbSerialWriteError> {
        let mut fmt_buf = core::mem::take(&mut self.fmt_buf);
        reserve_at_least(&mut fmt_buf, DEFAULT_FORMAT_BUF_SIZE)
            .map_err(|_| UsbSerialWriteError::AllocFailed)?;
        fmt_buf.clear();
        {
            let mut sink = FallibleFmtSink(&mut fmt_buf);
            fmt::write(&mut sink, args).map_err(|_| UsbSerialWriteError::AllocFailed)?;
        }
        push_try_str(&mut fmt_buf, "\r\n").map_err(|_| UsbSerialWriteError::AllocFailed)?;
        let res = self.write_bytes(fmt_buf.as_bytes()).await;
        self.fmt_buf = fmt_buf;
        res
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

struct FallibleFmtSink<'a>(&'a mut String);

impl fmt::Write for FallibleFmtSink<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        push_try_str(self.0, s).map_err(|_| fmt::Error)
    }
}

fn push_try_str(dst: &mut String, s: &str) -> Result<(), ()> {
    dst.try_reserve(s.len()).map_err(|_| ())?;
    dst.push_str(s);
    Ok(())
}

fn reserve_at_least(dst: &mut String, min_capacity: usize) -> Result<(), ()> {
    if dst.capacity() < min_capacity {
        dst.try_reserve(min_capacity - dst.capacity())
            .map_err(|_| ())?;
    }
    Ok(())
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
