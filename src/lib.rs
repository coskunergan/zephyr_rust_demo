// Copyright (c) 2024 Linaro LTD
// SPDX-License-Identifier: Apache-2.0

#![no_std]
// Sigh. The check config system requires that the compiler be told what possible config values
// there might be.  This is completely impossible with both Kconfig and the DT configs, since the
// whole point is that we likely need to check for configs that aren't otherwise present in the
// build.  So, this is just always necessary.

//#![allow(unexpected_cfgs)]
#![allow(warnings)]

use log::warn;
extern crate alloc;

use embassy_time::{Duration, Timer};

use alloc::boxed::Box;
use alloc::vec::Vec;

use zephyr::{
    device::gpio::{GpioPin, GpioToken},
    raw::{GPIO_INPUT, GPIO_OUTPUT_ACTIVE, GPIO_PULL_DOWN},
    sync::{Arc, Condvar, Mutex},
};

use embassy_executor::Executor;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use log::info;
use static_cell::StaticCell;

pub mod led;

static EXECUTOR_MAIN: StaticCell<Executor> = StaticCell::new();

pub static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();

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
    log::info!("GPIO token olusturuldu");

    let led_pin1 = zephyr::devicetree::labels::led::get_instance().unwrap();
    let led_pin2 = zephyr::devicetree::labels::led_red::get_instance().unwrap();

    log::info!("LED pinleri alindi: led_pin1 ve led_pin2");

    declare_leds!(
        spawner,
        gpio_token,
        [
            (led_pin1, Duration::from_millis(100)),
            (led_pin2, Duration::from_millis(500))
        ]
    );
    log::info!("LED'ler baslatildi");

    //spawner.spawn(blinky(spawner, gpio_token.clone())).unwrap();
    //spawner.spawn(button(spawner, gpio_token.clone())).unwrap();

    loop {
        Timer::after(Duration::from_millis(1000)).await;
    }
}
//////////////////////////////////////////////////////////////////////////////////
////////////////////////////////// BLINKY ////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
#[embassy_executor::task]
async fn blinky(spawner: Spawner, gpio_token: Arc<Mutex<zephyr::device::gpio::GpioToken>>) {
    info!("Hello world");
    let _ = spawner;
    let mut gpio_token_lock = gpio_token.lock().unwrap();

    warn!("Inside of blinky");

    let mut led = zephyr::devicetree::labels::led::get_instance().unwrap();

    if !led.is_ready() {
        warn!("LED is not ready");
        loop {}
    }

    unsafe {
        led.configure(&mut gpio_token_lock, GPIO_OUTPUT_ACTIVE);
    }

    loop {
        let val = BUTTON_SIGNAL.wait().await;
        //let val =  BUTTON_SIGNAL.wait_timeout(Duration::from_millis(1000)).await;

        if val == true {
            unsafe {
                led.toggle_pin(&mut gpio_token_lock);
            }
        }

        Timer::after(Duration::from_millis(200)).await;
    }
}
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////// BUTTON //////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
#[embassy_executor::task]
async fn button(spawner: Spawner, gpio_token: Arc<Mutex<zephyr::device::gpio::GpioToken>>) {
    info!("Hello world");
    let _ = spawner;
    let mut gpio_token = gpio_token.lock().unwrap();
    let mut led_red = zephyr::devicetree::labels::led_red::get_instance().unwrap();
    let mut button = zephyr::devicetree::labels::button::get_instance().unwrap();

    if !button.is_ready() {
        warn!("Button is not ready");
        loop {}
    }

    unsafe {
        button.configure(&mut gpio_token, GPIO_INPUT | GPIO_PULL_DOWN);
        led_red.configure(&mut gpio_token, GPIO_OUTPUT_ACTIVE);
    }

    loop {
        unsafe { button.wait_for_high(&mut gpio_token).await };

        unsafe {
            led_red.toggle_pin(&mut gpio_token);
        }

        Timer::after(Duration::from_millis(30)).await;

        BUTTON_SIGNAL.signal(true);
    }
}
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
