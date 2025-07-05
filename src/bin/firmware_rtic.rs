//! Read https://rtic.rs to learn more about the framework.

#![no_main]
#![no_std]

use {defmt_rtt as _, panic_probe as _};

#[rtic::app(device = stm32h7xx_hal::pac, peripherals = true, dispatchers = [EXTI0, EXTI1])]
mod app {
    use daisy_kickstart::{filter::FilterParams, processor::Processor};
    use heapless::spsc::{Consumer, Producer, Queue};

    use stm32h7xx_hal::prelude::*;
    use stm32h7xx_hal::{
        adc::AdcSampleTime,
        delay::DelayFromCountDownTimer,
        gpio::{Analog, Pin},
    };
    use systick_monotonic::Systick;
    use {
        daisy::pac::ADC1,
        defmt_rtt as _, panic_probe as _,
        stm32h7xx_hal::adc::{self, Adc, Enabled},
    };

    use cortex_m::Peripherals as CorePeripherals;
    use daisy::{audio::Interface, pac::Peripherals as DevicePeripherals};

    #[monotonic(binds = SysTick, default = true)]
    type Mono = Systick<1000>; // 1 kHz / 1 ms granularity

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        audio_interface: Interface,
        processor: Processor,
        inputs: Inputs,
        params_producer: Producer<'static, FilterParams, 8>,
        params_consumer: Consumer<'static, FilterParams, 8>,
    }

    #[init(
        local = [
            param_queue: Queue<FilterParams, 8> = Queue::new(),
        ]
    )]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        let system = System::init(cx.core, cx.device);
        let audio_interface = system.audio_interface;
        let mono = system.mono;
        let inputs = system.inputs;

        let (params_producer, params_consumer) = cx.local.param_queue.split();
        let processor = Processor::new();

        input::spawn().unwrap();

        (
            Shared {},
            Local {
                audio_interface,
                processor,
                inputs,
                params_producer,
                params_consumer,
            },
            init::Monotonics(mono),
        )
    }

    // Audio is tranfered from the input and to the input periodically thorugh DMA.
    // Every time Daisy is done transferring data, it will ask for more by triggering
    // the DMA 1 Stream 1 interrupt.
    #[task(binds = DMA1_STR1, local = [audio_interface, processor, params_consumer])]
    fn dsp(cx: dsp::Context) {
        let audio_interface = cx.local.audio_interface;
        let processor = cx.local.processor;
        let params_consumer = cx.local.params_consumer;

        // get the last item in the queue
        let mut params = None;
        while let Some(p) = params_consumer.dequeue() {
            params = Some(p);
        }

        // update the processor if there was something in the queue
        if let Some(params) = params {
            processor.update(params);
        }

        // process audio
        audio_interface
            .handle_interrupt_dma1_str1(|audio_buffer| {
                processor.process(audio_buffer);
            })
            .unwrap();
    }

    #[task(
        local = [
            inputs,
            params_producer,
        ],
        priority = 2,
    )]
    fn input(cx: input::Context) {
        input::spawn_after(systick_monotonic::ExtU64::millis(1))
            .ok()
            .unwrap();

        let inputs = cx.local.inputs;
        let params_producer = cx.local.params_producer;

        let _ = params_producer.enqueue(inputs.filter_params());
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

    struct System {
        pub mono: Systick<1000>,
        pub inputs: Inputs,
        pub audio_interface: Interface,
    }

    impl System {
        fn init(mut cp: CorePeripherals, dp: DevicePeripherals) -> Self {
            // Using caches should provide a major performance boost.
            cp.SCB.enable_icache();
            // NOTE: Data caching requires cache management around all use of DMA.
            // This crate already handles that for audio processing.
            cp.SCB.enable_dcache(&mut cp.CPUID);

            let board = daisy::Board::take().unwrap();
            let ccdr = daisy::board_freeze_clocks!(board, dp);
            let pins = daisy::board_split_gpios!(board, ccdr, dp);
            // let sdram = daisy::board_split_sdram!(cp, dp, ccdr, pins);
            let audio_interface = daisy::board_split_audio!(ccdr, pins);
            // Start audio processing and put its abstraction into a global.
            let audio_interface = audio_interface.spawn().unwrap();

            let mono = Systick::new(cp.SYST, ccdr.clocks.sys_ck().to_Hz());
            let mut delay = DelayFromCountDownTimer::new(dp.TIM2.timer(
                100.Hz(),
                ccdr.peripheral.TIM2,
                &ccdr.clocks,
            ));
            let mut adc1 = adc::Adc::adc1(
                dp.ADC1,
                4.MHz(),
                &mut delay,
                ccdr.peripheral.ADC12,
                &ccdr.clocks,
            )
            .enable();
            adc1.set_resolution(adc::Resolution::SixteenBit);
            adc1.set_sample_time(AdcSampleTime::T_16);

            // Select a pin that will be used for ADC, depending on the board.
            let adc1_channel = pins.GPIO.PIN_21.into_analog();
            let adc2_channel = pins.GPIO.PIN_15.into_analog();

            let inputs = Inputs {
                adc1,
                pot1_pin: adc1_channel,
                pot2_pin: adc2_channel,
            };

            Self {
                mono,
                inputs,
                audio_interface,
            }
        }
    }
}
