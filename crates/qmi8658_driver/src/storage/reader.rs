use crate::types::ImuFrame;

use super::ImuReader;

impl<'a> ImuReader<'a> {
    pub fn read_latest_frame(&self) -> ImuFrame {
        ImuFrame {
            sample: self.pipeline.latest_sample(),
        }
    }

    pub fn read_latest_temp(&self) -> i32 {
        self.pipeline.latest_temp()
    }
}
