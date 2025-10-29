
pub struct FrameHandler {
    producer: media::frame::Producer,
}

impl FrameHandler {
    pub fn new(producer: media::frame::Producer) -> Self {
        Self { producer }
    }

    pub fn something(&mut self) -> Result<media::frame::Frame, media::frame::ProducerError> {
        self.producer.fetch_frame()
    }
}