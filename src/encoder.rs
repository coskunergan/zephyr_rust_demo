// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

use embassy_time::{Duration, Timer};

use alloc::boxed::Box;
use zephyr::{
    raw::GPIO_INPUT,
    sync::{Arc, Mutex},
};

use super::{GpioPin, GpioToken};
use log::warn;

pub struct Encoder {
    token: Arc<Mutex<GpioToken>>,
    pin_a: GpioPin,
    pin_b: GpioPin,
    callback: Box<dyn Fn(bool) + Send + Sync + 'static>,
    debounce: Duration,
}

impl Encoder {
    pub fn new(
        token: Arc<Mutex<GpioToken>>,
        pin_a: GpioPin,
        pin_b: GpioPin,
        callback: Box<dyn Fn(bool) + Send + Sync + 'static>,
        debounce: Duration,
    ) -> Self {
        Self {
            token,
            pin_a,
            pin_b,
            callback,
            debounce,
        }
    }

    pub async fn work(&mut self) {
        let mut token_lock = self.token.lock().unwrap();

        if !self.pin_a.is_ready() || !self.pin_b.is_ready() {
            warn!("Encoder pins is not ready");
            loop {}
        }

        unsafe {
            self.pin_a.configure(&mut token_lock, GPIO_INPUT);
            self.pin_b.configure(&mut token_lock, GPIO_INPUT);
        }

        loop {
            let mut state = 0;

            unsafe { self.pin_a.wait_for_low(&mut token_lock).await };

            if unsafe { self.pin_b.get(&mut token_lock) } == false {
                state += 1;
            } else {
                state -= 1;
            }

            Timer::after(self.debounce).await;

            unsafe { self.pin_a.wait_for_high(&mut token_lock).await };

            if unsafe { self.pin_b.get(&mut token_lock) } == true {
                state += 1;
            } else {
                state -= 1;
            }

            Timer::after(self.debounce).await;

            match state {
                2 => (self.callback)(true),
                -2 => (self.callback)(false),
                _ => {
                    warn!("Unexpected state value: {}", state)
                }
            };
        }
    }
}

#[macro_export]
macro_rules! declare_encoders {
    ($spawner:expr, $token:expr, [ $( ($pin_a:expr,$pin_b:expr, $closure:expr, $debounce:expr) ),* ]) => {
        {
            const ENC_COUNT: usize = 0 $( + { let _ = ($debounce); 1 } )*;
            log::info!("Declared Encoder count: {}", ENC_COUNT);

            #[embassy_executor::task(pool_size = ENC_COUNT)]
            async fn encoder_task(mut encoder: crate::encoder::Encoder) {
                encoder.work().await;
            }

            $(
                use alloc::boxed::Box;
                let pin_a = $pin_a;
                let pin_b = $pin_b;
                let debounce = $debounce;
                let encoder = $crate::encoder::Encoder::new($token.clone(), pin_a, pin_b, Box::new($closure), debounce);
                match $spawner.spawn(encoder_task(encoder)) {
                    Ok(_) => log::info!("Encoder task started."),
                    Err(e) => log::error!("Encoder task failure: {:?}", e),
                }
            )*
        }
    };
}
