#![no_std]
#![no_main]

use core::mem::MaybeUninit;

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Timer};
use embedded_alloc::LlffHeap as Heap;
use panic_probe as _;
use static_cell::StaticCell;

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

#[global_allocator]
static HEAP: Heap = Heap::empty();
const USB_PRINT_HEAP_SIZE: usize = 2048;
static mut USB_PRINT_HEAP_MEM: [MaybeUninit<u8>; USB_PRINT_HEAP_SIZE] =
    [MaybeUninit::uninit(); USB_PRINT_HEAP_SIZE];
static mut DISPLAY_FRAMEBUFFER: co5300_driver::FrameBuffer = co5300_driver::FrameBuffer::new();
static IMU_PIPELINE_CELL: StaticCell<qmi8658_driver::ImuPipeline> = StaticCell::new();

fn init_usb_print_allocator() {
    let heap_start = core::ptr::addr_of_mut!(USB_PRINT_HEAP_MEM) as *mut MaybeUninit<u8> as usize;
    unsafe {
        HEAP.init(heap_start, USB_PRINT_HEAP_SIZE);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    init_usb_print_allocator();

    let p = embassy_rp::init(Default::default());
    let mut display = co5300_driver::Co5300::new_default(
        p.SPI1,
        p.DMA_CH0,
        p.PIN_9,
        p.PIN_10,
        p.PIN_11,
        p.PIN_15,
    );
    display.init_default().await;
    let framebuffer = unsafe { &mut *core::ptr::addr_of_mut!(DISPLAY_FRAMEBUFFER) };
    framebuffer.fill_rgb565(0x001F);
    display.write_framebuffer(framebuffer).await;

    let imu_pipeline = IMU_PIPELINE_CELL.init(qmi8658_driver::ImuPipeline::new());
    let imu = qmi8658_driver::Qmi8658::new_default(p.I2C1, p.PIN_6, p.PIN_7, p.PIN_8).unwrap();
    spawner.spawn(qmi8658_driver::imu_capture_task(imu, imu_pipeline).unwrap());
    let mut imu_reader = imu_pipeline.reader();

    let mut class = usb_serial::init(spawner, p.USB, usb_serial::UsbSerialConfig::default());
    let mut serial = usb_serial::UsbTextWriter::new(&mut class);

    serial.wait_connection().await;
    let _ = usb_serial::usb_println!(serial, "BOOT,display_ready");

    let mut buf = [0u8; 64];

    loop {
        match select(
            serial.read_packet(&mut buf),
            Timer::after(Duration::from_millis(100)),
        )
        .await
        {
            Either::First(Ok(n)) => {
                if n > 0 {
                    let _ = serial.write_packet(&buf[..n]).await;
                }
            }
            Either::First(Err(_)) => {}
            Either::Second(()) => {
                let frame = imu_reader.read_latest_frame();
                if frame.seq != 0 {
                    let tilt = frame.sample.tilt_deg_from_accel_8g();
                    let _ = usb_serial::usb_println!(serial, "{}", tilt);
                }
            }
        }
    }
}
