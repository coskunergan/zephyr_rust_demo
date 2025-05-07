// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

#![no_std]
//#![allow(warnings)]

extern crate alloc;

use embassy_time::{Duration, Timer};

use alloc::format;

#[cfg(feature = "executor-thread")]
use embassy_executor::Executor;

#[cfg(feature = "executor-zephyr")]
use zephyr::embassy::Executor;

use zephyr::{
    device::gpio::{GpioPin, GpioToken},
    sync::{Arc, Mutex},
};

use core::{sync::atomic::AtomicBool, sync::atomic::AtomicI32, sync::atomic::Ordering};
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use static_cell::StaticCell;

use adc_io::Adc;
use dac_io::Dac;
use display_io::Display;

mod adc_io;
mod button;
mod dac_io;
mod display_io;
mod encoder;
mod led;

static EXECUTOR_MAIN: StaticCell<Executor> = StaticCell::new();
pub static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
pub static ENCODER_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
static BL_STATE: AtomicBool = AtomicBool::new(false);
static COUNT: AtomicI32 = AtomicI32::new(0);
static DISPLAY: spin::Once<Display> = spin::Once::new();
static DAC: spin::Once<Dac> = spin::Once::new();

#[no_mangle]
extern "C" fn rust_main() {
    unsafe {
        zephyr::set_logger().unwrap();
    }

    let executor = EXECUTOR_MAIN.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(main(spawner)).unwrap();
    })
}

#[embassy_executor::task]
async fn main(spawner: Spawner) {
    let gpio_token = Arc::new(Mutex::new(unsafe { GpioToken::get_instance().unwrap() }));
    let led_red = zephyr::devicetree::labels::led_red::get_instance().unwrap();
    let led_green = zephyr::devicetree::labels::led_green::get_instance().unwrap();
    let led_blue = zephyr::devicetree::labels::led_blue::get_instance().unwrap();
    let led_orange = zephyr::devicetree::labels::led_orange::get_instance().unwrap();

    let display = Display::new();
    DISPLAY.call_once(|| display);

    let dac = Dac::new();
    DAC.call_once(|| dac);

    let mut adc = Adc::new();
    adc.read_async(
        core::time::Duration::from_millis(500),
        Some(|idx, value| {
            zephyr::printk!("ADC Channel {}: {}\n", idx, value);
            if idx == 0 {
                if let Some(dac) = DAC.get() {
                    dac.write(value as i32);
                }

                if let Some(display) = DISPLAY.get() {
                    display.clear();
                    let msg = format!("ADC {}: {}", idx, value);
                    display.write(msg.as_bytes());
                }
            }
        }),
    );

    declare_leds!(
        spawner,
        gpio_token,
        [
            (led_red, Duration::from_millis(75)),
            (led_green, Duration::from_millis(150)),
            (led_blue, Duration::from_millis(300)),
            (led_orange, Duration::from_millis(600))
        ]
    );

    let button = zephyr::devicetree::labels::button::get_instance().unwrap();

    declare_buttons!(
        spawner,
        gpio_token,
        [(
            button,
            || {
                zephyr::printk!("Button Pressed!");

                if let Some(display) = DISPLAY.get() {
                    display.clear();
                    BL_STATE.store(!BL_STATE.load(Ordering::SeqCst), Ordering::SeqCst);
                    display.set_backlight(BL_STATE.load(Ordering::SeqCst) as u8);
                }

                BUTTON_SIGNAL.signal(true);
            },
            Duration::from_millis(100)
        )]
    );

    let encoder_a = zephyr::devicetree::labels::encoder_a::get_instance().unwrap();
    let encoder_b = zephyr::devicetree::labels::encoder_b::get_instance().unwrap();

    declare_encoders!(
        spawner,
        gpio_token,
        [(
            encoder_a,
            encoder_b,
            |clockwise| {
                let mut value = COUNT.load(Ordering::SeqCst);
                if clockwise {
                    value += 1;
                } else {
                    value -= 1;
                }
                COUNT.store(value, Ordering::Release);
                ENCODER_SIGNAL.signal(clockwise);
            },
            Duration::from_millis(1)
        )]
    );

    loop {
        Timer::after(Duration::from_millis(30)).await;

        let msg: alloc::string::String = format!("Encoder: {}", COUNT.load(Ordering::SeqCst));

        if let Some(display) = DISPLAY.get() {
            display.clear();
            display.write(msg.as_bytes());
        }

        zephyr::printk!("{}\n", msg);

        ENCODER_SIGNAL.wait().await;
    }
}
