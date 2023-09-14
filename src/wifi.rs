use anyhow::Result;
use embedded_svc::wifi::*;
use esp_idf_hal::delay::Delay;
use esp_idf_hal::peripheral;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::mdns::EspMdns;
use esp_idf_svc::wifi::*;
use log::*;
use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WiFiState {
    Stopped,
    Started,
    Connected,
    IfUp,
}

pub struct WiFiConnection<'a, F>
where
    F: FnMut(WiFiState),
{
    esp_wifi: EspWifi<'static>,
    mdns: EspMdns,
    host: &'a str,
    ap_ssid: &'a str,
    ap_pass: &'a str,
    saved_ap_list: Option<HashMap<String, String>>,
    scanned_ap_list: Option<Vec<String>>,
    state: WiFiState,
    handler: Option<F>,
}

impl<'a, F> WiFiConnection<'a, F>
where
    F: FnMut(WiFiState),
{
    pub fn new(
        modem: impl peripheral::Peripheral<P = esp_idf_hal::modem::Modem> + 'static,
        sysloop: EspSystemEventLoop,
        host: &'a str,
        ap_ssid: &'a str,
        ap_pass: &'a str,
    ) -> Result<Self> {
        let nvs = None;
        let esp_wifi = EspWifi::<'static>::new(modem, sysloop.clone(), nvs)?;
        let mdns = EspMdns::take()?;
        Ok(Self {
            esp_wifi,
            mdns,
            host,
            ap_ssid,
            ap_pass,
            saved_ap_list: None,
            scanned_ap_list: None,
            state: WiFiState::Stopped,
            handler: None,
        })
    }

    pub fn wifi_start(
        &mut self,
        default_ssid: Option<&str>,
        saved_ap_list: Option<HashMap<String, String>>,
    ) -> Result<()> {
        self.saved_ap_list = saved_ap_list;

        let _ = self.esp_wifi.disconnect();
        let _ = self.esp_wifi.stop();

        self.esp_wifi
            .set_configuration(&Configuration::Client(ClientConfiguration::default()))?;
        self.esp_wifi.start()?;

        let ap_info = self.ap_scan();

        let candidate = ap_info.iter().find(|&a| {
            if let Some(ssid) = default_ssid {
                a.ssid == ssid
            } else {
                self.saved_ap_list.is_some()
                    && self
                        .saved_ap_list
                        .as_ref()
                        .unwrap()
                        .contains_key(&a.ssid.to_string())
            }
        });

        let ap_conf = AccessPointConfiguration {
            ssid: self.ap_ssid.into(),
            ssid_hidden: false,
            password: self.ap_pass.into(),
            auth_method: AuthMethod::WPA2Personal,
            ..Default::default()
        };

        let mut stn_conf = ClientConfiguration::default();

        if let Some(ap) = candidate {
            let ssid = ap.ssid.as_str();
            let passwd = self
                .saved_ap_list
                .as_ref()
                .unwrap()
                .get(ssid)
                .unwrap()
                .as_str();
            stn_conf = ClientConfiguration {
                ssid: ssid.into(),
                password: passwd.into(),
                channel: Some(ap.channel),
                ..Default::default()
            }
        };

        let conf = Configuration::Mixed(stn_conf, ap_conf);
        info!("Config = {:?}", conf);
        self.esp_wifi.set_configuration(&conf).unwrap();
        self.esp_wifi.start().unwrap();

        self.mdns.set_hostname(self.host)?;

        info!("Staring WiFi.");
        Ok(())
    }

    pub fn ap_scan(&mut self) -> Vec<AccessPointInfo> {
        let mut ap_info = self.esp_wifi.scan().unwrap();
        ap_info.sort_by(|a, b| b.signal_strength.cmp(&a.signal_strength));
        let aplist = ap_info.iter().map(|ap| ap.ssid.to_string()).collect();
        self.scanned_ap_list = if !ap_info.is_empty() {
            Some(aplist)
        } else {
            None
        };
        ap_info
    }

    pub fn scanned_ap_list(
        &mut self,
        saved_ap_list: Option<HashMap<String, String>>,
    ) -> Option<Vec<String>> {
        self.saved_ap_list = saved_ap_list;
        if self.scanned_ap_list.is_some() {
            let res = self
                .scanned_ap_list
                .as_ref()
                .unwrap()
                .iter()
                .map(|ap| {
                    if self.saved_ap_list.is_some()
                        && self.saved_ap_list.as_ref().unwrap().contains_key(ap)
                    {
                        format!("o {}", ap)
                    } else {
                        format!("x {}", ap)
                    }
                })
                .collect();
            Some(res)
        } else {
            None
        }
    }

    pub fn add_handler(&mut self, f: F) {
        self.handler = Some(f);
    }

    pub fn wifi_loop(&mut self) -> Result<()> {
        let prev = self.state;
        let mut delay = 200;

        if self.esp_wifi.is_started().is_ok() {
            self.state = WiFiState::Started;
            if let Ok(connected) = self.esp_wifi.is_connected() {
                if connected {
                    self.state = WiFiState::Connected;
                    delay = 500;
                    if self.esp_wifi.is_up().unwrap() {
                        self.state = WiFiState::IfUp;
                    }
                }
            }
        }

        if prev != self.state {
            if self.handler.is_some() {
                self.handler.as_mut().unwrap()(self.state);
                if self.state == WiFiState::IfUp {
                    let ap_info = self.esp_wifi.ap_netif().get_ip_info();
                    let sta_info = self.esp_wifi.sta_netif().get_ip_info();
                    info!("AP Info: {:?}\nSTN Info: {:?}", ap_info, sta_info);
                }
            }
        } else if self.state == WiFiState::Started {
            let _ = self.esp_wifi.connect();
        }
        Delay::delay_ms(delay);
        Ok(())
    }
}
