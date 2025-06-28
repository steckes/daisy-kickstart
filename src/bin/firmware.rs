//! Audio passthrough example with heapless queue for knob messages using direct ADC readings

#![no_main]
#![no_std]

use core::cell::RefCell;
use cortex_m::asm;
use cortex_m::interrupt::Mutex;
use cortex_m_rt::entry;

use daisy::audio;
use daisy_kickstart::filter::{Filter, FilterParams, FilterType};
use hal::adc;
use hal::delay::Delay;
use hal::pac::{self, interrupt};
use hal::prelude::*;
use stm32h7xx_hal as hal;

use {defmt_rtt as _, panic_probe as _};

// Global audio interface
static AUDIO_INTERFACE: Mutex<RefCell<Option<audio::Interface>>> = Mutex::new(RefCell::new(None));

static FILTER: Mutex<RefCell<Option<[Filter; 2]>>> = Mutex::new(RefCell::new(None));

// Global queue for knob messages
static CUTOFF: Mutex<RefCell<f32>> = Mutex::new(RefCell::new(440.0));
static RESONANCE: Mutex<RefCell<f32>> = Mutex::new(RefCell::new(0.71));

const MIN_CUTOFF: f32 = 20.0; // 20 Hz
const MAX_CUTOFF: f32 = 20000.0; // 20 kHz

const MIN_RESONANCE: f32 = 0.1;
const MAX_RESONANCE: f32 = 6.0;

// Simple moving average for smoothing
const SMOOTH_FACTOR: f32 = 0.9;

#[entry]
fn main() -> ! {
    // Acquire peripherals
    let mut cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // Enable caches
    cp.SCB.enable_icache();
    cp.SCB.enable_dcache(&mut cp.CPUID);

    // Initialize board
    let board = daisy::Board::take().unwrap();
    let ccdr = daisy::board_freeze_clocks!(board, dp);
    let pins = daisy::board_split_gpios!(board, ccdr, dp);

    // Configure ADC
    let mut delay = Delay::new(cp.SYST, ccdr.clocks);
    let mut adc1 = adc::Adc::adc1(
        dp.ADC1,
        4.MHz(),
        &mut delay,
        ccdr.peripheral.ADC12,
        &ccdr.clocks,
    )
    .enable();
    adc1.set_resolution(adc::Resolution::SixteenBit);
    // Select ADC channel for the knob
    let mut knob1_pin = pins.GPIO.PIN_21.into_analog();
    let mut knob2_pin = pins.GPIO.PIN_15.into_analog();

    // Spawn audio interface
    let audio_interface = daisy::board_split_audio!(ccdr, pins).spawn().unwrap();

    // Initialize filters
    let mut filters = [
        Filter::new(FilterType::Lowpass),
        Filter::new(FilterType::Lowpass),
    ];

    filters[0]
        .set_sample_rate(daisy::audio::FS.to_Hz() as f32)
        .unwrap();
    filters[1]
        .set_sample_rate(daisy::audio::FS.to_Hz() as f32)
        .unwrap();

    // Set initial filter parameters
    let cutoff_freq = 440.0; // 1kHz cutoff frequency (adjust as needed)
    let resonance = 0.71; // Resonance/Q factor (adjust as needed)
    let gain = 1.0;
    // Update filter parameters
    filters[0]
        .set_params(FilterParams {
            frequency: cutoff_freq,
            quality: resonance,
            gain,
        })
        .unwrap();
    filters[1]
        .set_params(FilterParams {
            frequency: cutoff_freq,
            quality: resonance,
            gain,
        })
        .unwrap();

    // Store interface and initialize queue
    cortex_m::interrupt::free(|cs| {
        AUDIO_INTERFACE.borrow(cs).replace(Some(audio_interface));
        FILTER.borrow(cs).replace(Some(filters));
    });

    // Main loop: read parameters
    loop {
        // Read ADC value
        let knob1_raw: u32 = adc1.read(&mut knob1_pin).unwrap();
        let knob2_raw: u32 = adc1.read(&mut knob2_pin).unwrap();

        // Convert 16-bit ADC (0..65535) to 0.0..1.0
        let knob1_norm = knob1_raw as f32 / 65_535.0;
        let knob2_norm = knob2_raw as f32 / 65_535.0;

        let new_cutoff = MIN_CUTOFF * libm::powf(MAX_CUTOFF / MIN_CUTOFF, knob1_norm);
        let new_resonance = MIN_RESONANCE + (MAX_RESONANCE - MIN_RESONANCE) * knob2_norm;

        cortex_m::interrupt::free(|cs| {
            let mut cutoff = CUTOFF.borrow(cs).borrow_mut();
            // smoothing
            *cutoff = *cutoff * SMOOTH_FACTOR + new_cutoff * (1.0 - SMOOTH_FACTOR);
            let mut resonance = RESONANCE.borrow(cs).borrow_mut();
            // smoothing
            *resonance = *resonance * SMOOTH_FACTOR + new_resonance * (1.0 - SMOOTH_FACTOR);
        });

        // wait for next interrupt
        cortex_m::asm::wfi();
    }
}

// DMA interrupt for audio processing
#[interrupt]
fn DMA1_STR1() {
    cortex_m::interrupt::free(|cs| {
        let cutoff = *CUTOFF.borrow(cs).borrow();
        let resonance = *RESONANCE.borrow(cs).borrow();

        // Process audio frames
        if let (Some(audio_interface), Some(filters)) = (
            AUDIO_INTERFACE.borrow(cs).borrow_mut().as_mut(),
            FILTER.borrow(cs).borrow_mut().as_mut(),
        ) {
            filters[0]
                .set_params(FilterParams {
                    frequency: cutoff,
                    quality: resonance,
                    gain: 1.0,
                })
                .unwrap();
            filters[1]
                .set_params(FilterParams {
                    frequency: cutoff,
                    quality: resonance,
                    gain: 1.0,
                })
                .unwrap();
            audio_interface
                .handle_interrupt_dma1_str1(|audio_buffer| {
                    for frame in audio_buffer {
                        let (left, right) = *frame;
                        let left = filters[0].tick(left);
                        let right = filters[1].tick(right);
                        *frame = (left, right);
                    }
                })
                .unwrap();
        }
    });
}
