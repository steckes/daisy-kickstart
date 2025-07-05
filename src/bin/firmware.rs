//! Audio passthrough example with heapless queue for knob messages using direct ADC readings

#![no_main]
#![no_std]

use core::cell::RefCell;
use cortex_m::Peripherals as CorePeripherals;
use cortex_m::interrupt::Mutex;
use cortex_m_rt::entry;
use daisy::pac::{ADC1, Peripherals as DevicePeripherals};

use daisy::audio::{self, Interface};
use daisy_kickstart::filter::FilterParams;
use daisy_kickstart::processor::Processor;
use hal::adc;
use hal::delay::Delay;
use hal::pac::{self, interrupt};
use hal::prelude::*;
use stm32h7xx_hal as hal;
use stm32h7xx_hal::adc::{Adc, Enabled};
use stm32h7xx_hal::gpio::{Analog, Pin};

use {defmt_rtt as _, panic_probe as _};

// Global Values
static AUDIO_INTERFACE: Mutex<RefCell<Option<audio::Interface>>> = Mutex::new(RefCell::new(None));

static PROCESSOR: Mutex<RefCell<Option<Processor>>> = Mutex::new(RefCell::new(None));

static PARAMS: Mutex<RefCell<FilterParams>> = Mutex::new(RefCell::new(FilterParams {
    frequency: 440.0,
    quality: 0.71,
    gain: 0.0,
}));

#[entry]
fn main() -> ! {
    // Acquire peripherals
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // Initialize system
    let system = System::init(cp, dp);
    let audio_interface = system.audio_interface;
    let mut inputs = system.inputs;

    // Initialize processor
    let processor = Processor::new();

    // Store interface and processor in global statics
    cortex_m::interrupt::free(|cs| {
        AUDIO_INTERFACE.borrow(cs).replace(Some(audio_interface));
        PROCESSOR.borrow(cs).replace(Some(processor));
    });

    // Main loop: read parameters
    loop {
        // Read ADC values
        let new_params = inputs.filter_params();

        // Update parameters
        cortex_m::interrupt::free(|cs| {
            let mut params = PARAMS.borrow(cs).borrow_mut();
            *params = new_params;
        });

        // Wait for next interrupt
        cortex_m::asm::wfi();
    }
}

// DMA interrupt for audio processing
#[interrupt]
fn DMA1_STR1() {
    cortex_m::interrupt::free(|cs| {
        // Process audio frames
        if let (Some(audio_interface), Some(processor), params) = (
            AUDIO_INTERFACE.borrow(cs).borrow_mut().as_mut(),
            PROCESSOR.borrow(cs).borrow_mut().as_mut(),
            PARAMS.borrow(cs).borrow(),
        ) {
            processor.update(params.clone());
            audio_interface
                .handle_interrupt_dma1_str1(|audio_buffer| {
                    processor.process(audio_buffer);
                })
                .unwrap();
        }
    });
}

struct System {
    pub inputs: Inputs,
    pub audio_interface: Interface,
}

impl System {
    fn init(mut cp: CorePeripherals, dp: DevicePeripherals) -> Self {
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

        // Select ADC channel for the pots
        let pot1_pin = pins.GPIO.PIN_21.into_analog();
        let pot2_pin = pins.GPIO.PIN_15.into_analog();

        // Spawn audio interface
        let audio_interface = daisy::board_split_audio!(ccdr, pins).spawn().unwrap();

        let inputs = Inputs {
            adc1,
            pot1_pin,
            pot2_pin,
        };

        Self {
            inputs,
            audio_interface,
        }
    }
}

struct Inputs {
    pub adc1: Adc<ADC1, Enabled>,
    pub pot1_pin: Pin<'C', 4, Analog>,
    pub pot2_pin: Pin<'C', 0, Analog>,
}

impl Inputs {
    fn filter_params(&mut self) -> FilterParams {
        let knob1_raw: u32 = self.adc1.read(&mut self.pot1_pin).unwrap();
        // Normalize 16-bit ADC (0..65535) to 0.0..1.0
        let knob1_norm = knob1_raw as f32 / 65_535.0;

        const MIN_FREQ: f32 = 20.0;
        const MAX_FREQ: f32 = 20_000.0;
        let frequency = MIN_FREQ * libm::powf(MAX_FREQ / MIN_FREQ, knob1_norm);

        let knob2_raw: u32 = self.adc1.read(&mut self.pot2_pin).unwrap();
        // Normalize 16-bit ADC (0..65535) to 0.0..1.0
        let knob2_norm = knob2_raw as f32 / 65_535.0;
        const MIN_Q: f32 = 0.1;
        const MAX_Q: f32 = 6.0;
        let quality = MIN_Q + (MAX_Q - MIN_Q) * knob2_norm;

        FilterParams {
            frequency,
            quality,
            gain: 0.0,
        }
    }
}
