use std::collections::VecDeque;

/// Кольцевой буфер аудио.
/// Хранит последние `capacity` сэмплов.
pub struct AudioRingBuffer {
    buffer: VecDeque<f32>,
    capacity: usize,
}

impl AudioRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        for &sample in samples {
            if self.buffer.len() >= self.capacity {
                self.buffer.pop_front();
            }

            self.buffer.push_back(sample);
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn snapshot(&self) -> Vec<f32> {
        self.buffer.iter().copied().collect()
    }

    /// Возвращает последние `samples` сэмплов.
    pub fn last_samples(&self, samples: usize) -> Vec<f32> {
        let count = samples.min(self.buffer.len());

        self.buffer
            .iter()
            .skip(self.buffer.len() - count)
            .copied()
            .collect()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}