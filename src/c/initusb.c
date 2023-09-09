#include "esp_vfs_dev.h"
#include "esp_vfs_usb_serial_jtag.h"
#include "driver/usb_serial_jtag.h"

void init_usb(void)
{
  usb_serial_jtag_driver_config_t usb_serial_jtag_config;
  usb_serial_jtag_config.rx_buffer_size = 128;
  usb_serial_jtag_config.tx_buffer_size = 128;
  usb_serial_jtag_driver_install(&usb_serial_jtag_config);
  esp_vfs_usb_serial_jtag_use_driver();
}
