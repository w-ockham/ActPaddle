use anyhow::{bail, Result};
use embedded_svc::wifi::*;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::peripheral;
use esp_idf_svc::mdns::EspMdns;
use esp_idf_svc::wifi::*;
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use log::*;

pub struct WiFiConnection<'a> {
    esp_wifi: EspWifi<'static>,
    mdns: EspMdns,
    host: Option<&'a str>,
    ssidlist: Option<Vec<String>>,
    ifup: bool,
}

impl<'a> WiFiConnection<'a> {
    pub fn new(
        modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
        sysloop: EspSystemEventLoop,
    ) -> Result<Self> {
        //let nvs = EspDefaultNvsPartition::take()?;
        let esp_wifi = EspWifi::<'static>::new(modem, sysloop.clone(), None /*Some(nvs)*/)?;
        let mdns = EspMdns::take()?;
        Ok(Self {
            esp_wifi,
            mdns,
            host: None,
            ssidlist: None,
            ifup: false,
        })
    }

    pub fn wifi_start(
        &mut self,
        host: &'a str,
        stn_ssid: &'a str,
        stn_pass: &'a str,
        ap_ssid: &'a str,
        ap_pass: &'a str,
    ) -> Result<()> {
        let _ = self.esp_wifi.disconnect();
        let _ = self.esp_wifi.stop();

        let ap_conf = AccessPointConfiguration {
            ssid: ap_ssid.into(),
            ssid_hidden: false,
            password: ap_pass.into(),
            auth_method: AuthMethod::WPA2Personal,
            ..Default::default()
        };

        let stn_conf = ClientConfiguration {
            ssid: stn_ssid.into(),
            password: stn_pass.into(),
            auth_method: AuthMethod::WPA2Personal,
            ..Default::default()
        };

        let conf = Configuration::Mixed(stn_conf, ap_conf);
        info!("Config = {:?}", conf);
        self.esp_wifi.set_configuration(&conf).unwrap();
        self.esp_wifi.start().unwrap();

        let ap_info = self.esp_wifi.scan().unwrap();
        info!("AP Info = {:?}",ap_info);
        self.ssidlist = Some(ap_info.into_iter().map(|ap| ap.ssid.to_string()).collect());
        self.host = Some(host);
        self.mdns.set_hostname(self.host.unwrap())?;

        info!("Staring WiFi.");
        Ok(())
    }

    pub fn get_ssidlist(&self) -> Option<Vec<String>> {
        self.ssidlist.clone()
    }

    pub fn wifi_loop(&mut self) -> Result<()> {
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
