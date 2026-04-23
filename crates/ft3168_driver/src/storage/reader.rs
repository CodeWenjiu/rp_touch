use crate::types::TouchFrame;

use super::TouchReader;

impl<'a> TouchReader<'a> {
    pub fn read_latest_frame(&self) -> TouchFrame {
        TouchFrame {
            sample: self.pipeline.latest_sample(),
        }
    }
}
