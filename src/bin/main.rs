#![no_std]
#![allow(unused)]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use alloc::boxed::Box;
use alloc::string::ToString;
use core::pin::pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::main;
use esp_hal::time::{Duration, Instant};
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use esp_radio::wifi::Config;
use esp_radio::wifi::sta::StationConfig;
// notes - still dont understand the whole sync part entirely

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
static WOKEN: AtomicBool = AtomicBool::new(true);

fn waker_wake(_: *const ()) {
    WOKEN.store(true, Ordering::SeqCst);
}

fn waker_clone(_: *const ()) -> RawWaker {
    make_raw_maker()
}

fn make_raw_maker() -> RawWaker {
    static VTABLE: RawWakerVTable =
        RawWakerVTable::new(waker_clone, waker_wake, waker_wake, |_| {});
    RawWaker::new(core::ptr::null(), &VTABLE)
}

fn block_on_sleepy<F: Future>(fut: F) -> F::Output {
    let mut fut = Box::pin(fut);
    let waker = unsafe { Waker::from_raw(make_raw_maker()) };
    let mut context = Context::from_waker(&waker);

    loop {
        let woken = critical_section::with(|_| {
            let w = WOKEN.load(Ordering::SeqCst);
            if w {
                WOKEN.store(false, Ordering::SeqCst);
            }
            w
        });

        if woken {
            if let Poll::Ready(val) = fut.as_mut().poll(&mut context) {
                return val;
            }
        }

        riscv::asm::wfi();
    }
}

#[main]
fn main() -> ! {
    println!("Booted!");
    const SSID: &str = env!("SSID");
    const PASSWORD: &str = env!("PASSWORD");

    //atomic bool since interrupts are asynchoronous and can happen even mid read

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);
    let (mut wifi_controller, interfaces) =
        esp_radio::wifi::new(peripherals.WIFI, Default::default())
            .expect("Failed to initialize Wi-Fi controller");

    let client_config = Config::Station(
        StationConfig::default()
            .with_ssid(SSID.to_string())
            .with_password(PASSWORD.to_string()),
    );

    wifi_controller
        .set_config(&client_config)
        .expect("Failed to configure");

    match block_on_sleepy(wifi_controller.connect_async()) {
        Ok(connected) => println!("{:?}", connected),
        Err(err) => println!("{:?}", err),
    }

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
    loop {
        riscv::asm::wfi();
    }
}
