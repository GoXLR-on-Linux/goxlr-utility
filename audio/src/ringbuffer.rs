use log::warn;
use rb::{Consumer, Producer, RB, RbConsumer, RbInspector, RbProducer, SpscRb};

/// This is a simple fixed-sized RingBuffer that permits overflowing
///
/// This is designed for situations where you want to continuously write to the buffer, then at
/// any time retrieve all data currently stored. Useful when handling an audio sample buffer!
pub struct RingBuffer<T> {
    buffer: SpscRb<T>,
    consumer: Consumer<T>,
    producer: Producer<T>,
}

impl<T: Clone + Copy + Default> RingBuffer<T> {
    pub fn new(size: usize) -> Self {
        let buffer = SpscRb::<T>::new(size);
        let (producer, consumer) = (buffer.producer(), buffer.consumer());

        Self {
            buffer,
            consumer,
            producer,
        }
    }

    pub fn write_into(&self, data: &[T]) -> anyhow::Result<()> {
        // If we have enough space available in the buffer, we don't need to do anything fancy
        if self.buffer.slots_free() > data.len() {
            self.producer.write(data).map_err(anyhow::Error::msg)?;
            return Ok(());
        }

        // Is the input data simply too large to fit into the buffer?
        if self.buffer.capacity() < data.len() {
            // First thing we need to do is force the consumer pointer to the 'end'
            self.consumer.skip_pending().map_err(anyhow::Error::msg)?;

            // Calculate where we need to 'start' in the data to be able to fit the tail
            let start = data.len() - self.buffer.capacity();
            let data = &data[start..data.len()];

            // Write this into the buffer
            self.producer.write(data).map_err(anyhow::Error::msg)?;

            return Ok(());
        }

        // In this final case, we don't have room in the buffer, but the data can fit
        // we simply need to shift the consumer so there's enough unread, and push in
        let skip = data.len() - self.buffer.slots_free();
        self.consumer.skip(skip).map_err(anyhow::Error::msg)?;
        self.producer.write(data).map_err(anyhow::Error::msg)?;

        Ok(())
    }

    pub fn read_buffer(&self) -> anyhow::Result<Vec<T>> {
        let mut buffer = Vec::new();
        buffer.resize_with(self.buffer.count(), Default::default);

        // Fill the buffer with data...
        let count = self.consumer.get(&mut buffer).map_err(anyhow::Error::msg)?;
        Ok(Vec::from(&buffer[0..count]))
    }

    #[allow(dead_code)]
    pub fn read_and_clear_buffer(&self) -> anyhow::Result<Vec<T>> {
        let mut buffer = Vec::new();
        buffer.resize_with(self.buffer.count(), Default::default);

        // Fill the buffer with data...
        let count = self
            .consumer
            .read(&mut buffer)
            .map_err(anyhow::Error::msg)?;
        Ok(Vec::from(&buffer[0..count]))
    }

    pub fn clear(&self) {
        // We simply need to skip forward
        if let Err(e) = self.consumer.skip_pending() {
            warn!("Error Skipping Buffer: {}", e);
        }
    }
}
