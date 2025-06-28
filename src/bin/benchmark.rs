#![no_main]
#![no_std]

use core::hint::black_box;

use daisy_kickstart::{
    bench_time,
    filter::{Filter, FilterType},
};

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::println!("Run Benchmark");

    // Get access to Cortex-M core peripherals (CPU-level hardware)
    let mut cortex_peripherals = cortex_m::Peripherals::take().unwrap();

    // Enable instruction and data caches for better performance
    cortex_peripherals.SCB.enable_icache();
    cortex_peripherals
        .SCB
        .enable_dcache(&mut cortex_peripherals.CPUID);

    // Create two arrays of 32 floating-point numbers, all set to 1.0
    let mut audio_buffer = [(1.0, 1.0); 32];

    // Get access to device-specific peripherals (board-level hardware)
    let device_peripherals = daisy::pac::Peripherals::take().unwrap();

    // Initialize the Daisy board
    let daisy_board = daisy::Board::take().unwrap();

    // Configure and freeze the clock settings for the board
    let clock_configuration = daisy::board_freeze_clocks!(daisy_board, device_peripherals);

    // Get the system clock frequency in Hz
    let system_clock_frequency_hz = clock_configuration.clocks.sys_ck().to_Hz();

    let mut filters = [
        Filter::new(FilterType::Lowpass),
        Filter::new(FilterType::Lowpass),
    ];

    // Benchmark the dot product calculation and measure execution time
    let execution_time = bench_time!(
        cortex_peripherals,
        {
            for frame in audio_buffer.iter_mut() {
                let (left, right) = *frame;

                // Apply filters to each channel
                let filtered_left = filters[0].tick(left);
                let filtered_right = filters[1].tick(right);

                // Update the frame with filtered audio
                *frame = (filtered_left, filtered_right);
            }
        },
        system_clock_frequency_hz,
        ns // Return result in microseconds (possible values: ms, us, ns)
    );

    defmt::println!("Time: {} ns", execution_time);

    // Loop infinite
    loop {
        cortex_m::asm::wfi(); // Wait for interrupt (low power)
    }
}
