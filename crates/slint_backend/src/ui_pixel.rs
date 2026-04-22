use slint::platform::software_renderer::{PremultipliedRgbaColor, Rgb565Pixel, TargetPixel};

#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub(crate) struct UiPixel(pub(crate) u16);

impl TargetPixel for UiPixel {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let mut native = Rgb565Pixel(u16::from_be(self.0));
        <Rgb565Pixel as TargetPixel>::blend(&mut native, color);
        self.0 = native.0.to_be();
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let native = <Rgb565Pixel as TargetPixel>::from_rgb(r, g, b);
        Self(native.0.to_be())
    }
}
