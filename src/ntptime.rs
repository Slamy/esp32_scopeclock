use core::cell::Cell;

use critical_section::Mutex;
use embassy_net::udp::{self, UdpSocket};
use embassy_net::{IpEndpoint, Stack};

use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_println::println;

use esp_wifi::wifi::{WifiDevice, WifiStaDevice};

use smoltcp::wire::DnsQueryType;
use sntpc::async_impl::get_time;
use sntpc::{NtpContext, NtpTimestampGenerator};

#[derive(Copy, Clone)]
pub struct StdTimestampGen {
    timestamp: embassy_time::Instant,
    offset: embassy_time::Duration,
}

impl NtpTimestampGenerator for StdTimestampGen {
    fn init(&mut self) {
        self.timestamp = embassy_time::Instant::now() + self.offset;
    }

    fn timestamp_sec(&self) -> u64 {
        self.timestamp.as_secs()
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.timestamp.as_micros() - self.timestamp.as_secs() * 1000000) as u32
    }
}

pub static PUBLIC_TIME: Mutex<Cell<Option<StdTimestampGen>>> = Mutex::new(Cell::new(None));

#[embassy_executor::task]
pub async fn time_stuff(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    let mut rx_buffer = [0; 1000];
    let mut tx_buffer = [0; 1000];
    let mut tx_meta = [udp::PacketMetadata::EMPTY, udp::PacketMetadata::EMPTY];
    let mut rx_meta = [udp::PacketMetadata::EMPTY, udp::PacketMetadata::EMPTY];

    let ntp_addr = loop {
        match stack
            .dns_query("0.de.pool.ntp.org", DnsQueryType::A)
            .await
            .map(|a| a[0])
        {
            Ok(address) => break address,
            Err(e) => {
                println!("DNS lookup error: {e:?}");
                Timer::after(Duration::from_millis(1000)).await;
                continue;
            }
        };
    };

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    // accept any local port
    socket.bind(0).unwrap();

    let mut ntp_context = NtpContext::new(StdTimestampGen {
        timestamp: embassy_time::Instant::now(),
        offset: Duration::from_secs(0),
    });
    println!("NTP connecting to {:?}", ntp_addr);

    let ntp_endpoint = IpEndpoint {
        addr: ntp_addr,
        port: 123,
    };

    // get first coarse measurement
    loop {
        let res = get_time(ntp_endpoint, &socket, ntp_context);
        let res = embassy_time::with_timeout(Duration::from_secs(3), res).await;

        match res {
            Ok(Ok(res)) => {
                let system_now = embassy_time::Instant::now().as_micros();
                ntp_context.timestamp_gen.offset = Duration::from_secs(res.seconds as u64)
                    + Duration::from_micros(((res.seconds_fraction as u64) * 1000000) >> 32)
                    - Duration::from_micros(system_now);
                println!(
                    "NTP {:?} {} {}",
                    res,
                    system_now,
                    ((res.seconds_fraction as u64) * 1000000) >> 32
                );
                break;
            }
            Ok(Err(res)) => {
                println!("NTP Error {:?}", res)
            }
            Err(res) => {
                println!("NTP Timeout {:?}", res);
            }
        }
    }

    //get next ones to refine the offset
    Timer::after(Duration::from_millis(1000)).await;

    for _ in 0..4 {
        let res = get_time(ntp_endpoint, &socket, ntp_context);
        let res = embassy_time::with_timeout(Duration::from_secs(3), res).await;

        match res {
            Ok(Ok(res)) => {
                println!("NTP2 {:?}", res);

                if res.offset > 0 {
                    ntp_context.timestamp_gen.offset += Duration::from_micros(res.offset as u64);
                } else {
                    ntp_context.timestamp_gen.offset -= Duration::from_micros((-res.offset) as u64);
                }
            }
            Ok(Err(res)) => {
                println!("NTP2 Error {:?}", res)
            }
            Err(res) => {
                println!("NTP2 Timeout {:?}", res);
            }
        }

        critical_section::with(|cs| {
            let cell = PUBLIC_TIME.borrow(cs);
            cell.set(Some(ntp_context.timestamp_gen));
        });

        Timer::after(Duration::from_millis(1000)).await;
    }

    // we should be synced now
    loop {
        let res = get_time(ntp_endpoint, &socket, ntp_context);
        let res = embassy_time::with_timeout(Duration::from_secs(3), res).await;

        match res {
            Ok(Ok(res)) => {
                println!("NTP3 {:?}", res);
            }
            Ok(Err(res)) => {
                println!("NTP3 Error {:?}", res);

                socket.close();
                drop(socket);

                socket = UdpSocket::new(
                    stack,
                    &mut rx_meta,
                    &mut rx_buffer,
                    &mut tx_meta,
                    &mut tx_buffer,
                );

                socket.bind(0).unwrap();
            }
            Err(res) => {
                println!("NTP3 Timeout {:?}", res);
            }
        }

        Timer::after(Duration::from_millis(10000)).await;
    }
}
