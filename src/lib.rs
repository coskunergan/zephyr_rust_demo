// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

#![no_std]

//#![allow(warnings)]

extern crate alloc;

use embassy_time::{Duration, Timer};

use alloc::boxed::Box;
use alloc::format;

#[cfg(feature = "executor-thread")]
use embassy_executor::Executor;

#[cfg(feature = "executor-zephyr")]
use zephyr::embassy::Executor;

use zephyr::{
    raw,
    device::gpio::{GpioPin, GpioToken},
    sync::{Arc, Mutex},
};

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use static_cell::StaticCell;

use crate::raw::__device_dts_ord_16;
use crate::raw::auxdisplay_backlight_set;
use crate::raw::auxdisplay_clear;
use crate::raw::auxdisplay_write;

use core::{
    sync::atomic::Ordering,
    sync::atomic::AtomicI32,
};

mod button;
mod encoder;
mod led;

static EXECUTOR_MAIN: StaticCell<Executor> = StaticCell::new();
pub static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
pub static ENCODER_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
static mut BL_STATE: bool = false;
static COUNT: AtomicI32 = AtomicI32::new(0);

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
                log::info!("Button Pressed!");
                unsafe {
                    auxdisplay_backlight_set(&__device_dts_ord_16, {
                        BL_STATE = !BL_STATE;
                        BL_STATE as u8
                    });
                };

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
                if clockwise 
                {
                    value +=1;
                }
                else
                {
                    value -=1;
                }
                COUNT.store(value, Ordering::Release);
                ENCODER_SIGNAL.signal(clockwise);                
            },
            Duration::from_millis(1)
        )]
    );    

    let lcd_device = unsafe { &__device_dts_ord_16 as *const crate::raw::device };

    loop {

        Timer::after(Duration::from_millis(100)).await;

        unsafe { auxdisplay_clear(lcd_device) };

        let msg = format!("Encoder: {}", COUNT.load(Ordering::SeqCst));

        unsafe { auxdisplay_write(lcd_device, msg.as_ptr(), msg.len().try_into().unwrap()) };
        
        log::info!("{}",msg);

        ENCODER_SIGNAL.wait().await;        
    }
}
