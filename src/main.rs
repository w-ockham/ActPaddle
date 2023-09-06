use core::ffi::c_char;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::PinDriver;
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::eventloop::*;
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::mdns::EspMdns;

use std::io::stdin;
use std::sync::mpsc;
use std::thread;

mod morse;
mod param;
mod server;
mod wifi;
use crate::morse::Morse;
use crate::param::KeyerParam;
use crate::server::spawn_server;
use crate::wifi::wifi_connect;
use log::*;

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
    #[default("")]
    hostname: &'static str,
}

#[cfg(target_arch = "riscv32")]
extern "C" {
    fn init_usb();
}

#[cfg(target_arch = "xtensa")]
fn init_uart0() {
    use core::ptr::null_mut;
    use esp_idf_sys::{
        esp_vfs_dev_uart_port_set_rx_line_endings, esp_vfs_dev_uart_use_driver, uart_driver_install,
    };
    unsafe {
        esp_vfs_dev_uart_port_set_rx_line_endings(
            0,
            esp_idf_sys::esp_line_endings_t_ESP_LINE_ENDINGS_CR,
        );
        esp_idf_sys::esp!(uart_driver_install(0, 256, 0, 0, null_mut(), 0))
            .expect("unable to initialize UART0 driver");
        esp_vfs_dev_uart_use_driver(0);
    }
}

fn patches() {
    esp_idf_sys::link_patches();
    use esp_idf_sys::esp_get_idf_version;
    use std::ffi::CStr;

    let c_buf: *const c_char = unsafe { esp_get_idf_version() };
    let c_str: &CStr = unsafe { CStr::from_ptr(c_buf) };
    println!("ESP-IDF version = {}", c_str.to_str().unwrap());

    #[cfg(target_arch = "riscv32")]
    unsafe {
        init_usb();
    }
    #[cfg(target_arch = "xtensa")]
    init_uart0();
}

static LOGGER: EspLogger = EspLogger;

fn main() -> anyhow::Result<()> {
    patches();

    log::set_logger(&LOGGER).map(|()| LOGGER.initialize())?;
    LOGGER.set_target_level("", log::LevelFilter::Info)?;

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;

    let di = PinDriver::output(peripherals.pins.gpio3)?;
    let dah = PinDriver::output(peripherals.pins.gpio2)?;

    let mut morse = Morse::new(di, dah);

    let (from_post, rx_post) = mpsc::channel::<KeyerParam>();
    let (from_serial, rx_serial) = mpsc::channel::<KeyerParam>();

    let mut wifi = wifi_connect(
        peripherals.modem,
        sysloop.clone(),
        CONFIG.wifi_ssid,
        CONFIG.wifi_psk,
    )?;

    let mut mdns = EspMdns::take()?;
    mdns.set_hostname(CONFIG.hostname)?;

    let _server = spawn_server(from_post);

    thread::spawn(move || {
        let reader = stdin();
        loop {
            let mut line = String::new();
            if let Err(e) = reader.read_line(&mut line) {
                print!("Error: {e}\r\n");
            } else {
                let mesg: Result<KeyerParam, serde_json::Error> = serde_json::from_str(&line);
                if let Ok(mesg) = mesg {
                    from_serial.send(mesg);
                } else {
                    print!("JSONError: {:?}", mesg);
                }
            }
            FreeRtos::delay_ms(10);
        }
    });

    morse.set_wpm(40);
    morse.play(true, &"OK".to_string());

    let mut connecting = false;

    loop {
        let mut interp = |param: KeyerParam| {
            if let Some(s) = param.wpm {
                morse.set_wpm(s);
            }

            if let Some(r) = param.ratio {
                morse.set_ratio(r);
            }

            if let Some(s) = param.letter_space {
                morse.set_letter_space(s);
            }

            if let Some(s) = param.word_space {
                morse.set_word_space(s);
            }

            if let Some(r) = param.reverse {
                if r {
                    morse.reverse();
                } else {
                    morse.normal();
                }
            }
            if let Some(m) = param.to_paddle {
                morse.play(true, &m);
            }

            if let Some(m) = param.to_straight {
                morse.play(false, &m);
            }
        };

        if let Ok(msg) = rx_post.try_recv() {
            interp(msg);
        }

        if let Ok(msg) = rx_serial.try_recv() {
            interp(msg);
        }

        if !wifi.is_connected().unwrap() && !connecting {
            connecting = true;
            if let Err(_) = wifi.connect() {
                connecting = false;
                info!("connect failed.");
            } else {
                info!("connecting");
                wifi.connect();
            }
        } else if wifi.is_connected().unwrap() && connecting {
            info!("connected");
            connecting = false;
        }

        FreeRtos::delay_ms(1);
    }
}
