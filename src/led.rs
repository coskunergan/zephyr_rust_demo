use embassy_time::{Duration, Timer};

use zephyr::{
    raw::GPIO_OUTPUT_ACTIVE,
    sync::{Arc, Mutex},
};

use super::{GpioPin, GpioToken};
use log::warn;

pub struct Led {
    token: Arc<Mutex<GpioToken>>,
    pin: GpioPin,
    delay: Duration,
}

impl Led {
    pub fn new(token: Arc<Mutex<GpioToken>>, pin: GpioPin, delay: Duration) -> Self {
        Self { token, pin, delay }
    }

    pub async fn blinky(&mut self) {
        let mut token_lock = self.token.lock().unwrap();

        if !self.pin.is_ready() {
            warn!("LED pin is not ready");
            loop {}
        }

        unsafe {
            self.pin.configure(&mut token_lock, GPIO_OUTPUT_ACTIVE);
        }

        loop {
            unsafe {
                self.pin.toggle_pin(&mut token_lock);
            }
            Timer::after(self.delay).await;
        }
    }
}

#[macro_export]
macro_rules! declare_leds {
    ($spawner:expr, $token:expr, [ $( ($pin:expr, $delay:expr) ),* ]) => {
        {
            const LED_COUNT: usize = 0 $( + { let _ = ($delay); 1 } )*;
            log::info!("Declared LED count: {}", LED_COUNT);

            #[embassy_executor::task(pool_size = LED_COUNT)]
            async fn led_task(mut led: crate::led::Led) {
                led.blinky().await;
            }

            $(
                let pin = $pin;
                let delay = $delay;
                let led = $crate::led::Led::new($token.clone(), pin, delay);
                match $spawner.spawn(led_task(led)) {
                    Ok(_) => log::info!("LED task started."),
                    Err(e) => log::error!("LED task failure: {:?}", e),
                }
            )*
        }
    };
}
