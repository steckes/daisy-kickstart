#![no_main]
#![no_std]

use daisy::audio::BLOCK_LENGTH;
use daisy_kickstart::{US, bench_time, processor::Processor};

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
    let mut audio_buffer = [(1.0, 1.0); BLOCK_LENGTH];

    // Get access to device-specific peripherals (board-level hardware)
    let device_peripherals = daisy::pac::Peripherals::take().unwrap();

    // Initialize the Daisy board
    let daisy_board = daisy::Board::take().unwrap();

    // Configure and freeze the clock settings for the board
    let clock_configuration = daisy::board_freeze_clocks!(daisy_board, device_peripherals);

    // Get the system clock frequency in Hz
    let system_clock_frequency_hz = clock_configuration.clocks.sys_ck().to_Hz();

    // Initialize the processor
    let mut processor = Processor::new();

    // Benchmark the dot product calculation and measure execution time
    let execution_time = bench_time!(cortex_peripherals, system_clock_frequency_hz, {
        processor.process(&mut audio_buffer);
    });

    let process_time = BLOCK_LENGTH as f32 / daisy::audio::FS.to_Hz() as f32;

    defmt::println!("Time: {} us", execution_time * US as f32);
    defmt::println!("Time available: {} us", process_time * US as f32);

    // Loop infinite
    loop {
        cortex_m::asm::wfi(); // Wait for interrupt (low power)
    }
}
