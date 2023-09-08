use anyhow::Result;
use embedded_svc::wifi::*;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::peripheral;
use esp_idf_svc::mdns::EspMdns;
use esp_idf_svc::wifi::*;
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use esp_idf_sys::EspError;

use log::*;

pub struct WiFiConnection<'a> {
    esp_wifi: EspWifi<'static>,
    mdns: EspMdns,
    host: Option<&'a str>,
    ifup: bool,
}

impl<'a> WiFiConnection<'a> {
    pub fn new(
        modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
        sysloop: EspSystemEventLoop,
    ) -> Self {
        let nvs = EspDefaultNvsPartition::take().unwrap();
        let esp_wifi = EspWifi::<'static>::new(modem, sysloop.clone(), Some(nvs)).unwrap();
        let mdns = EspMdns::take().unwrap();
        Self {
            esp_wifi,
            mdns,
            host: None,
            ifup: false,
        }
    }

    pub fn wifi_start_stn(
        &mut self,
        host: &'a str,
        ssid: &'a str,
        pass: &'a str,
    ) -> Result<(), EspError> {
        let _ = self.esp_wifi.disconnect();
        let _ = self.esp_wifi.stop();

        self.esp_wifi
            .set_configuration(&Configuration::Client(ClientConfiguration {
                ssid: ssid.into(),
                password: pass.into(),
                auth_method: AuthMethod::WPA2Personal,
                ..Default::default()
            }))?;

        self.esp_wifi.start()?;
        self.host = Some(host);
        self.mdns.set_hostname(self.host.unwrap())?;

        info!("Staring WiFi station.");
        Ok(())
    }

    pub fn wifi_start_ap(
        &mut self,
        host: &'a str,
        ssid: &'a str,
        pass: &'a str,
    ) -> Result<(), EspError> {
        let _ = self.esp_wifi.disconnect();
        let _ = self.esp_wifi.stop();

        self.esp_wifi
            .set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
                ssid: ssid.into(),
                ssid_hidden: false,
                auth_method: AuthMethod::WPA2Personal,
                password: pass.into(),
                ..Default::default()
            }))?;

        self.esp_wifi.start()?;
        self.host = Some(host);
        self.mdns.set_hostname(self.host.unwrap())?;

        info!("Starting WiFi AP.");
        Ok(())
    }

    pub fn wifi_loop(&mut self) -> Result<(), EspError> {
        if self.esp_wifi.is_started().is_ok() {
            if let Ok(connected) = self.esp_wifi.is_connected() {
                if !connected {
                    if self.esp_wifi.connect().is_err() {
                        FreeRtos::delay_ms(20);
                        self.ifup = false;
                    }
                } else if self.esp_wifi.is_up().unwrap() && !self.ifup {
                    let ap_info = self.esp_wifi.ap_netif().get_ip_info();
                    let sta_info = self.esp_wifi.sta_netif().get_ip_info();
                    info!("AP Info: {:?}\nSTN Info: {:?}", ap_info, sta_info);
                    self.ifup = true;
                }
            }
        }
        Ok(())
    }
}
