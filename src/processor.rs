use daisy::audio::BLOCK_LENGTH;

use crate::filter::{Filter, FilterType};

pub use crate::filter::FilterParams;

pub struct Processor {
    filter_left: Filter,
    filter_right: Filter,
}

impl Processor {
    pub fn new() -> Self {
        let mut filter_left = Filter::new(FilterType::Lowpass);
        let mut filter_right = Filter::new(FilterType::Lowpass);
        filter_left
            .set_sample_rate(daisy::audio::FS.to_Hz() as f32)
            .unwrap();
        filter_right
            .set_sample_rate(daisy::audio::FS.to_Hz() as f32)
            .unwrap();
        Self {
            filter_left,
            filter_right,
        }
    }

    pub fn update(&mut self, params: FilterParams) {
        self.filter_left.set_params(params).unwrap();
        self.filter_right.set_params(params).unwrap();
    }

    pub fn process(&mut self, audio_buffer: &mut [(f32, f32); BLOCK_LENGTH]) {
        for (left, right) in audio_buffer.iter_mut() {
            *left = self.filter_left.tick(*left);
            *right = self.filter_right.tick(*right);
        }
    }
}
