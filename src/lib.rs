// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN

#![no_std]
// Sigh. The check config system requires that the compiler be told what possible config values
// there might be.  This is completely impossible with both Kconfig and the DT configs, since the
// whole point is that we likely need to check for configs that aren't otherwise present in the
// build.  So, this is just always necessary.

//#![allow(warnings)]

extern crate alloc;

use embassy_time::{Duration, Timer};

use alloc::boxed::Box;

use crate::raw::__device_dts_ord_16;
use crate::raw::auxdisplay_write;

use zephyr::raw;

#[cfg(feature = "executor-thread")]
use embassy_executor::Executor;

#[cfg(feature = "executor-zephyr")]
use zephyr::embassy::Executor;

use zephyr::{
    device::gpio::{GpioPin, GpioToken},
    sync::{Arc, Mutex},
};

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use static_cell::StaticCell;

mod button;
mod encoder;
mod led;

static EXECUTOR_MAIN: StaticCell<Executor> = StaticCell::new();

pub static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();
pub static ENCODER_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();

//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
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
//////////////////////////////////////////////////////////////////////////////////
////////////////////////////////// MAIN //////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
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
            (led_red, Duration::from_millis(100)),
            (led_green, Duration::from_millis(200)),
            (led_blue, Duration::from_millis(400)),
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
                ENCODER_SIGNAL.signal(clockwise);
            },
            Duration::from_millis(5)
        )]
    );

    Timer::after(Duration::from_millis(100)).await;

    //let lcd_device = unsafe { zephyr::devicetree::labels::auxdisplay_0::get_instance_raw() };
    //let lcd_device = unsafe { zephyr::devicetree::labels::aux_display_gpio::get_instance_raw() }; /*__device_dts_ord_15 */
    let lcd_device = unsafe { &__device_dts_ord_16 as *const crate::raw::device };

    let message = "coskunergan.dev";

    let rc = unsafe {
        auxdisplay_write(
            lcd_device,
            message.as_ptr(),
            message.len().try_into().unwrap(),
        )
    };

    if rc != 0 {
        log::warn!("Failed to lcd write {}", rc);
    }

    let mut count = 0;
    loop {
        if ENCODER_SIGNAL.wait().await as bool {
            count += 1;
        } else {
            count -= 1;
        }
        log::info!("Encoder Value: {}", count);
    }
}
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
