use daisy::audio::BLOCK_LENGTH;

use crate::audio_processor::filter::Filter;

pub mod filter;

pub struct AudioProcessor {
    filter_left: Filter,
    filter_right: Filter,
}

impl AudioProcessor {
    pub fn new() -> Self {
        let mut filter_left = Filter::new(filter::FilterType::Lowpass);
        let mut filter_right = Filter::new(filter::FilterType::Lowpass);
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

    // Process function
    pub fn process(&mut self, audio_block: &mut [(f32, f32); BLOCK_LENGTH]) {
        for (left, right) in audio_block.iter_mut() {
            *left = self.filter_left.tick(*left);
            *right = self.filter_right.tick(*right);
        }
    }
}
