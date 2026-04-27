use crate::{
    regs::{
        CTRL9_CMD_REQ_FIFO, FIFO_CTRL_RD_MODE, QMI8658_REG_AX_L, QMI8658_REG_FIFO_CTRL,
        QMI8658_REG_FIFO_SMPL_CNT, QMI8658_REG_FIFO_STATUS,
    },
    types::{Error, ImuRawSample, ImuReport, Int1FifoStreamState},
};

use super::Qmi8658;

impl<'d> Qmi8658<'d> {
    pub async fn read_accel_gyro_raw(&mut self) -> Result<ImuRawSample, Error> {
        let mut buf = [0u8; 12];
        self.read_regs(QMI8658_REG_AX_L, &mut buf).await?;
        Ok(Self::parse_sample(&buf))
    }

    pub async fn read_fifo_samples_into(
        &mut self,
        out: &mut [ImuRawSample],
    ) -> Result<usize, Error> {
        self.ctrl9_command(CTRL9_CMD_REQ_FIFO).await?;

        let sample_words = self.fifo_word_count().await?;
        let total_bytes = sample_words as usize * 2;
        let total_samples = total_bytes / 12;
        let trailing_bytes = total_bytes % 12;

        let mut sample_buf = [0u8; 12];
        let mut written = 0usize;
        for index in 0..total_samples {
            self.read_fifo_bytes(&mut sample_buf).await?;
            if index < out.len() {
                out[index] = Self::parse_sample(&sample_buf);
                written += 1;
            }
        }

        if trailing_bytes > 0 {
            let mut discard = [0u8; 12];
            self.read_fifo_bytes(&mut discard[..trailing_bytes]).await?;
        }

        self.write_reg(
            QMI8658_REG_FIFO_CTRL,
            self.fifo_ctrl_cfg & !FIFO_CTRL_RD_MODE,
        )
        .await?;
        Ok(written)
    }

    pub async fn poll_int1_fifo_report(
        &mut self,
        _state: &mut Int1FifoStreamState,
        fifo_batch: &mut [ImuRawSample],
    ) -> Result<usize, ImuReport> {
        let pending_words = match self.fifo_word_count().await {
            Ok(v) => v,
            Err(_) => return Err(ImuReport::ReadError),
        };

        if pending_words == 0 {
            // No FIFO payload ready yet. This is a normal runtime condition
            // and should not be treated as a transport error.
            return Ok(0);
        }

        match self.read_fifo_samples_into(fifo_batch).await {
            Ok(n) if n > 0 => Ok(n),
            Ok(_) | Err(_) => Err(ImuReport::ReadError),
        }
    }

    fn parse_sample(buf: &[u8; 12]) -> ImuRawSample {
        ImuRawSample {
            accel: [
                i16::from_le_bytes([buf[0], buf[1]]),
                i16::from_le_bytes([buf[2], buf[3]]),
                i16::from_le_bytes([buf[4], buf[5]]),
            ],
            gyro: [
                i16::from_le_bytes([buf[6], buf[7]]),
                i16::from_le_bytes([buf[8], buf[9]]),
                i16::from_le_bytes([buf[10], buf[11]]),
            ],
        }
    }

    async fn fifo_word_count(&mut self) -> Result<u16, Error> {
        let sample_words_lsb = self.read_reg(QMI8658_REG_FIFO_SMPL_CNT).await?;
        let fifo_status = self.read_reg(QMI8658_REG_FIFO_STATUS).await?;
        Ok((((fifo_status & 0b11) as u16) << 8) | sample_words_lsb as u16)
    }
}
