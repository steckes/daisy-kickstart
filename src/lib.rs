#![no_main]
#![no_std]

use daisy::hal as _;
use defmt_rtt as _;
use panic_probe as _;

pub mod filter;
pub mod processor;

pub const MS: u32 = 1_000;
pub const US: u32 = 1_000_000;
pub const NS: u32 = 1_000_000_000;

// Custom panic handler to avoid duplicate panic messages
// Uses defmt for formatted logging instead of standard panic behavior
#[defmt::panic_handler]
fn panic() -> ! {
    cortex_m::asm::udf() // Trigger undefined instruction exception
}

/// Terminates the application gracefully for probe-run debugger
/// Makes the debugger exit with success status (exit-code = 0)
pub fn exit() -> ! {
    loop {
        cortex_m::asm::bkpt(); // Trigger breakpoint instruction repeatedly
    }
}

/// Measures CPU cycles taken to execute an expression on ARM Cortex-M
///
/// # Arguments
/// * `$cp` - Core peripherals (must have DCB and DWT access)
/// * `$x` - Expression to measure
///
/// # Returns
/// Number of CPU cycles as u32
///
/// # Panics
/// Panics if cycle counter cannot be enabled
///
/// # Example
/// ```
/// let cycles = op_cyccnt_diff!(cp, {
///     // code to measure
/// });
/// ```
#[macro_export]
macro_rules! bench_cycles {
    ( $cp:expr, $x:expr ) => {
        {
            use core::sync::atomic::{self, Ordering};
            use daisy::pac::DWT;

            $cp.DCB.enable_trace();
            $cp.DWT.enable_cycle_counter();

            atomic::compiler_fence(Ordering::Acquire);
            let before = DWT::cycle_count();
            $x
            let after = DWT::cycle_count();
            atomic::compiler_fence(Ordering::Release);

            if after >= before {
                after - before
            } else {
                after + (u32::MAX - before)
            }
        }
    };
}

/// Measures execution time of an expression using DWT cycle counter
///
/// Converts CPU cycles to time units based on system clock frequency.
///
/// # Arguments
/// * `$cp` - Cortex peripherals (cortex_m::Peripherals)
/// * `$sysclk_hz` - System clock frequency in Hz
/// * `$x` - Expression to measure
///
/// # Returns
/// Execution time as `f32` in seconds
///
/// # Example
/// ```rust
/// let cp = cortex_m::Peripherals::take().unwrap();
/// let time_s = bench_time!(cp, 480_000_000, {
///     for i in 0..1000 { cortex_m::asm::nop(); }
/// });
/// ```
///
/// # Notes
/// - Requires ARM Cortex-M with DWT support
/// - 32-bit cycle counter wraps after ~10.7s at 400MHz
/// - Minimal overhead but some measurement artifacts exist
#[macro_export]
macro_rules! bench_time {
    ( $cp:expr, $sysclk_hz:expr, $x:expr ) => {{
        let cycles = $crate::bench_cycles!($cp, $x);
        (cycles as f32) / ($sysclk_hz as f32)
    }};
}
