use embassy_net::tcp::TcpSocket;

use embassy_net::Stack;

use embassy_time::{Duration, Timer};
use esp_backtrace as _;

use esp_println::println;

use esp_wifi::wifi::{WifiDevice, WifiStaDevice};

use embassy_futures::select::{select, Either};
use rust_mqtt::{
    client::{client::MqttClient, client_config::ClientConfig},
    packet::v5::reason_codes::ReasonCode,
    utils::rng_generator::CountingRng,
};
use smoltcp::wire::DnsQueryType;

use crate::scopeclock;

#[embassy_executor::task]
pub async fn mqtt_stuff(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    let mut rx_buffer = [0; 1000];
    let mut tx_buffer = [0; 1000];
    loop {
        Timer::after(Duration::from_millis(1_000)).await;

        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        let address = match stack
            .dns_query("slamy", DnsQueryType::A)
            .await
            .map(|a| a[0])
        {
            Ok(address) => address,
            Err(e) => {
                println!("DNS lookup error: {e:?}");
                continue;
            }
        };

        //let address = Ipv4Address([192, 168, 178, 32]);
        let remote_endpoint = (address, 1883);
        println!("MQTT connecting to {:?}", remote_endpoint);
        let connection = socket.connect(remote_endpoint).await;
        if let Err(e) = connection {
            println!("MQTT connect error: {:?}", e);
            continue;
        }
        println!("MQTT connected!");

        let mut config = ClientConfig::new(
            rust_mqtt::client::client_config::MqttVersion::MQTTv5,
            CountingRng(20000),
        );
        config.add_max_subscribe_qos(rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1);

        config.add_client_id("clientId-8rhWgBODCl");
        config.max_packet_size = 100;
        let mut recv_buffer = [0; 80];
        let mut write_buffer = [0; 80];

        let mut client =
            MqttClient::<_, 5, _>::new(socket, &mut write_buffer, 80, &mut recv_buffer, 80, config);

        match client.connect_to_broker().await {
            Ok(()) => {}
            Err(mqtt_error) => match mqtt_error {
                ReasonCode::NetworkError => {
                    println!("MQTT Network Error");
                    continue;
                }
                _ => {
                    println!("Other MQTT Error: {:?}", mqtt_error);
                    continue;
                }
            },
        }

        client.subscribe_to_topic("beam_off").await.unwrap();
        client.subscribe_to_topic("beam_on").await.unwrap();

        loop {
            // TODO There is a big issue here. rust-mqtt by obabec is flawed
            // https://github.com/obabec/rust-mqtt/issues/38
            // https://github.com/obabec/rust-mqtt/issues/23
            // workaround inspired by https://github.com/obabec/rust-mqtt/issues/36
            // The workaround is still not helping and data might get lost.
            // It only helps keeping the connection alive as a ping is sent.
            match select(
                client.receive_message(),
                Timer::after(Duration::from_secs(2)),
            )
            .await
            {
                Either::First(msg) => match msg {
                    Ok(("beam_off", param)) => {
                        if let Ok(s) = core::str::from_utf8(param) {
                            let p = s.parse::<u32>().unwrap();
                            println!("Beam off: {}", p);
                            scopeclock::WAIT_BEFORE_BEAM_OFF
                                .store(p, core::sync::atomic::Ordering::Relaxed)
                        }
                    }
                    Ok(("beam_on", param)) => {
                        if let Ok(s) = core::str::from_utf8(param) {
                            let p = s.parse::<u32>().unwrap();
                            println!("Beam on: {}", p);
                            scopeclock::WAIT_AFTER_BEAM_ON
                                .store(p, core::sync::atomic::Ordering::Relaxed)
                        }
                    }
                    Ok((topic, param)) => {
                        println!("Unexpected topic {}: {:?}", topic, param);
                    }
                    Err(mqtt_error) => {
                        println!("Other MQTT Error: {:?}", mqtt_error);
                    }
                },
                Either::Second(_timeout) => {
                    client.send_ping().await.unwrap();
                }
            }

            Timer::after(Duration::from_millis(100)).await;

            /*
            // Convert temperature into String
            let temperature_string = format!("Howdy {num}");
            num += 1;
            match client
                .send_message(
                    "temperature/1",
                    temperature_string.as_bytes(),
                    rust_mqtt::packet::v5::publish_packet::QualityOfService::QoS1,
                    true,
                )
                .await
            {
                Ok(()) => {}
                Err(mqtt_error) => match mqtt_error {
                    ReasonCode::NetworkError => {
                        println!("MQTT Network Error");
                    }
                    ReasonCode::NoMatchingSubscribers => {
                        // Just ignore that
                    }
                    _ => {
                        println!("Other MQTT Error: {:?}", mqtt_error);
                    }
                },
            }
            Timer::after(Duration::from_millis(3000)).await;

             */
        }
    }
}
