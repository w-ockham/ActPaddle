use anyhow::Result;
use embedded_svc::http::Method;
use embedded_svc::io::Write;
use esp_idf_svc::http::server::*;
use esp_idf_svc::tls::X509;

use crate::param::KeyerParam;
use log::*;
use std::include_bytes;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

pub fn spawn_server(
    tx: Sender<KeyerParam>,
    rx: Receiver<KeyerParam>,
) -> Result<Box<EspHttpServer>> {
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
            let mut mesg  = String::new();
            if let Ok(size) = req.read(&mut buffer[0..255]) {
                let body = String::from_utf8(buffer[0..size].to_vec()).unwrap();
                let param = serde_json::from_str::<KeyerParam>(&body);
                if let Ok(js) = param {
                    info!("Posted value: {:?} = {:?}", body, js);
                    if js.ssidlist.is_some() {
                        tx.send(js)?;
                        if let Ok(ssids) = rx.recv_timeout(Duration::from_secs(10)) {
                          mesg = serde_json::to_string(&ssids).unwrap();
                        }
                    } else {
                        tx.send(js)?;
                    }
                } else {
                    info!("JSON format error: {:?}", body);
                }
            } else {
                info!("Request read error.");
            }
            req.into_response(200, Some("OK"), cors_headers)?.write_all(mesg.as_bytes())?;
            Ok(())
        })?;
    Ok(server)
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

fn index_html() -> String {
      r##"
    <html>
  <head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width">
    <title>ActPaddle</title>
  </head>
    <body bgcolor="#BB86FC">
    <h4> ActPaddle WiFi client configrations.</h4>
    <table>
    <tr>
      <td><label for="ssid">SSID:</label></td>
      <td><select name="ssid" id="ssid">
	        <option disabled selected>Select SSID</option>
      </select></td>
    </tr>
    <tr>
      <td><label for="ssid">Password:</label></td>
      <td><input type="password" id="passwd" size="16"></td>
    </tr>
    <tr>
      <td><label for="ssid">Retype:</label></td>
      <td><input type="password" id="passwd2" size="16"></td>
      <td><button onclick="submit()">OK</button></td>
    </tr>
    </table>
    <script>
    async function scan() {
	let api = location.protocol + '//'
	    + location.hostname + '/play';
	let ssidlist = document.getElementById("ssid");
  ssidlist.disabled = false;
  ssidlist.innerHTML = "";
  let jsonstr = JSON.stringify({'ssidlist':[]});
	res = await fetch(api, {
        method:'POST',
        headers: { 'Content-Type': 'application/json' },
        body: jsonstr
  });
	res = await res.json();
	for (const s of res['ssidlist']) {
	    let opt = document.createElement("option");
	    opt.innerHTML = s;
	    ssidlist.appendChild(opt);
	  };
  }
  
  async function submit() {
	  let api = location.protocol
	      + '//' + location.hostname + '/play';
	  let jsonmsg = {};
	  let ssidlist = document.getElementById("ssid");
	  let passwd = document.getElementById("passwd");
	  let passwd2 = document.getElementById("passwd2");
    if (passwd.value != passwd2.value) {
        window.alert("Different password.");
        return;
    }
	  jsonmsg["ssid"] = ssidlist.value;
	  jsonmsg["password"] = passwd.value;
    if (window.confirm("Change access point to SSID: \""+ssidlist.value+"\"\nPress OK to proceed.")) {
	    let jsonstr = JSON.stringify(jsonmsg);
	    await fetch(api, {
	        method: 'POST',
	        headers: {
		        'Content-Type': 'applicatoin/json'
	        },
	        body: jsonstr
	    })
      .catch((err) => { window.alert('Connection Error:'+uri+' '+err)});
    };
  }
  scan();
  </script>
</body>
</html>

    "##.to_string()
}
