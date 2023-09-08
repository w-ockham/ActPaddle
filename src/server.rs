use anyhow::Result;
use embedded_svc::http::Method;
use embedded_svc::io::Write;
use esp_idf_svc::http::server::*;
use esp_idf_svc::tls::X509;

use crate::param::KeyerParam;
use log::*;
use std::include_bytes;
use std::sync::mpsc::Sender;

pub fn spawn_server(tx: Sender<KeyerParam>) -> Result<Box<EspHttpServer>> {
    let server_cert_bytes: Vec<u8> = include_bytes!("../cert/cacert.pem").to_vec();
    let private_key_bytes: Vec<u8> = include_bytes!("../cert/prvtkey.pem").to_vec();

    let server_cert: X509 = convert_certificate(server_cert_bytes);
    let private_key: X509 = convert_certificate(private_key_bytes);

    let mut server_config = Configuration::default();

    let cors_headers = &[
        ("Access-Control-Allow-Methods", "POST"),
        ("Access-Control-Allow-Origin", "*"),
        ("Access-Control-Allow-Headers", "*"),
    ];

    server_config.server_certificate = Some(server_cert);
    server_config.private_key = Some(private_key);
    let mut server = Box::new(EspHttpServer::new(&server_config)?);

    server
        .fn_handler("/", Method::Get, |req| {
            let html = index_html();
            req.into_ok_response()?.write_all(html.as_bytes())?;
            Ok(())
        })?
        .fn_handler("/play", Method::Options, move |req| {
            req.into_response(204, None, cors_headers)?
                .write("".as_bytes())?;
            Ok(())
        })?
        .fn_handler("/play", Method::Post, move |mut req| {
            let mut buffer = [0u8; 256];
            if let Ok(size) = req.read(&mut buffer[0..255]) {
                let body = String::from_utf8(buffer[0..size].to_vec()).unwrap();
                let param = serde_json::from_str(&body);
                if let Ok(jstr) = param {
                    info!("Posted value: {:?}", jstr);
                    tx.send(jstr)?;
                } else {
                    info!("JSON format error: {:?}", body);
                }
            } else {
              info!("Request read error.");
            }
            req.into_response(200, None, cors_headers)?;
            Ok(())
        })?;
    Ok(server)
}

fn templated(content: impl AsRef<str>) -> String {
    format!(
        r#"
<!DOCTYPE html>
<html>
  <head>
      <meta charset="utf-8">
      <title>esp-rs web server</title>
  </head>
  <body>
      {}
  </body>
</html>
"#,
        content.as_ref()
    )
}

fn index_html() -> String {
    templated("Please post keyer message to /play")
}

fn convert_certificate(mut certificate_bytes: Vec<u8>) -> X509<'static> {
    // append NUL
    certificate_bytes.push(0);

    // convert the certificate
    let certificate_slice: &[u8] = unsafe {
        let ptr: *const u8 = certificate_bytes.as_ptr();
        let len: usize = certificate_bytes.len();
        std::mem::forget(certificate_bytes);

        std::slice::from_raw_parts(ptr, len)
    };
    // return the certificate file in the correct format
    X509::pem_until_nul(certificate_slice)
}
