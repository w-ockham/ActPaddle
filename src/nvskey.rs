use anyhow::{bail, Result};
use esp_idf_svc::nvs::*;
use log::*;
use std::collections::HashMap;
pub struct NVSkey {
    nvs: EspNvs<NvsDefault>,
    max_keys: u8,
}

impl NVSkey {
    pub fn new(namespace: &str) -> Result<NVSkey> {
        let nvs_default_partition = EspDefaultNvsPartition::take()?;
        let nvs = match EspNvs::new(nvs_default_partition, namespace, true) {
            Ok(nvs) => nvs,
            Err(e) => bail!("Could't get namespace {:?}", e),
        };
        Ok(NVSkey { nvs, max_keys: 4 })
    }

    pub fn clear(&mut self) -> Result<()> {
        for n in 0..self.max_keys {
            let ssidkey = format!("ssid{}", n);
            let passwdkey = format!("passwd{}", n);
            self.nvs.set_str(&ssidkey, "")?;
            self.nvs.set_str(&passwdkey, "")?;
        }
        info!("Clear NVS memory.");
        Ok(())
    }

    pub fn get_ssid_list(&mut self) -> Option<HashMap<String, String>> {
        let mut result = HashMap::new();
        for n in 0..self.max_keys {
            let ssidkey = format!("ssid{}", n);
            let passwdkey = format!("passwd{}", n);
            if let Some(ssid) = self.get_value(&ssidkey) {
                if !ssid.is_empty() {
                    result.insert(ssid, self.get_value(&passwdkey).unwrap());
                }
            }
        }
        if !result.is_empty() {
            Some(result)
        } else {
            None
        }
    }

    pub fn del_ssid(&mut self, ssid: &str) -> Result<()> {
        for n in 0..self.max_keys {
            let ssidkey = format!("ssid{}", n);
            let passwdkey = format!("passwd{}", n);
            if let Some(nvs_ssid) = self.get_value(&ssidkey) {
                if nvs_ssid == ssid {
                    info!("Clear SSID");
                    self.nvs.set_str(&ssidkey, "")?;
                    self.nvs.set_str(&passwdkey, "")?;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    pub fn set_ssid(&mut self, new_ssid: &str, new_passwd: &str) -> Result<()> {
        for n in 0..self.max_keys {
            let ssidkey = format!("ssid{}", n);
            let passwdkey = format!("passwd{}", n);
            if let Some(ssid) = self.get_value(&ssidkey) {
                if ssid.is_empty() {
                    self.nvs.set_str(&ssidkey, new_ssid)?;
                    self.nvs.set_str(&passwdkey, new_passwd)?;
                    info!("Set new SSID {}", new_ssid);
                    return Ok(());
                } else if ssid == new_ssid {
                    if !new_passwd.is_empty() {
                        self.nvs.set_str(&passwdkey, new_passwd)?;
                    }
                    return Ok(());
                }
            } else {
                self.nvs.set_str(&ssidkey, new_ssid)?;
                self.nvs.set_str(&passwdkey, new_passwd)?;
                return Ok(());
            }
        }
        bail!("No NVS avalilable")
    }

    pub fn get_value(&self, key: &str) -> Option<String> {
        const MAX_STR_LEN: usize = 256;
        if self.nvs.str_len(key).unwrap().is_some() {
            let mut buffer: [u8; MAX_STR_LEN] = [0; MAX_STR_LEN];
            if let Some(v) = self.nvs.get_str(key, &mut buffer).unwrap() {
                Some(v.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }
}
