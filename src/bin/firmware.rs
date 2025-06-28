//! Example of how to configure audio input output and implement a basic
//! filter processing on left and right channels, using RTIC.
//!
//! Read https://rtic.rs to learn more about the framework.

#![no_main]
#![no_std]

use {defmt_rtt as _, panic_probe as _};

#[rtic::app(device = stm32h7xx_hal::pac, peripherals = true)]
mod app {
    use daisy::audio::Interface;
    use systick_monotonic::*;

    // Import filter types - adjust this based on your filter implementation
    use daisy_kickstart::filter::{Filter, FilterParams, FilterType}; // Replace with actual import

    #[monotonic(binds = SysTick, default = true)]
    type Mono = Systick<1000>; // 1 kHz / 1 ms granularity

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        audio_interface: Interface,
        filters: [Filter; 2], // Array of 2 filters for stereo processing
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        // Get device peripherals.
        let mut cp = cx.core;
        let dp = cx.device;

        // Using caches should provide a major performance boost.
        cp.SCB.enable_icache();
        // NOTE: Data caching requires cache management around all use of DMA.
        // This crate already handles that for audio processing.
        cp.SCB.enable_dcache(&mut cp.CPUID);

        // Initialize the board abstraction.
        let board = daisy::Board::take().unwrap();

        // Configure board's peripherals.
        let ccdr = daisy::board_freeze_clocks!(board, dp);
        let pins = daisy::board_split_gpios!(board, ccdr, dp);
        let audio_interface = daisy::board_split_audio!(ccdr, pins);

        // Start audio processing and put its abstraction into a global.
        let audio_interface = audio_interface.spawn().unwrap();

        // Initialize filters
        let mut filters = [
            Filter::new(FilterType::Lowpass),
            Filter::new(FilterType::Lowpass),
        ];

        // Set initial filter parameters
        let cutoff_freq = 10_000.0; // 1kHz cutoff frequency (adjust as needed)
        let resonance = 0.7; // Resonance/Q factor (adjust as needed)

        // Update filter parameters
        filters[0]
            .set_params(FilterParams {
                frequency: cutoff_freq,
                quality: resonance,
                gain: 1.0,
            })
            .unwrap();
        filters[1]
            .set_params(FilterParams {
                frequency: cutoff_freq,
                quality: resonance,
                gain: 1.0,
            })
            .unwrap();

        // Initialize monotonic timer.
        let mono = Systick::new(cp.SYST, ccdr.clocks.sys_ck().to_Hz());

        (
            Shared {},
            Local {
                audio_interface,
                filters,
            },
            init::Monotonics(mono),
        )
    }

    // Audio is transferred from the input and to the output periodically through DMA.
    // Every time Daisy is done transferring data, it will ask for more by triggering
    // the DMA 1 Stream 1 interrupt.
    #[task(binds = DMA1_STR1, local = [audio_interface, filters])]
    fn dsp(cx: dsp::Context) {
        let audio_interface = cx.local.audio_interface;
        let filters = cx.local.filters;

        audio_interface
            .handle_interrupt_dma1_str1(|audio_buffer| {
                // Process each frame through the filters
                for frame in audio_buffer {
                    let (left, right) = *frame;

                    // Apply filters to each channel
                    let filtered_left = filters[0].tick(left);
                    let filtered_right = filters[1].tick(right);

                    // Update the frame with filtered audio
                    *frame = (filtered_left, filtered_right);
                }
            })
            .unwrap();
    }
}
