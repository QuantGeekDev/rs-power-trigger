use anyhow::*;
use esp_idf_hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::FromValueType;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use heapless::String;
use std::net::UdpSocket;
use std::str::FromStr;

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Starting app!");

    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sysloop.clone(), Some(nvs))?,
        sysloop,
    )?;

    let ssid: String<32> = String::from("Sercomm6DE4".parse().expect("Invalid ssd"));
    let password: String<64> = String::from("NLCGNEQFKKJ7LH".parse().expect("Invalid password"));

    wifi.set_configuration(&esp_idf_svc::wifi::Configuration::Client(
        esp_idf_svc::wifi::ClientConfiguration {
            ssid,
            password,
            ..Default::default()
        },
    ))?;

    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;
    log::info!("IP info: {:?}", ip_info);

    let socket = UdpSocket::bind(format!("{}:12345", ip_info.ip))?;

    let config = TimerConfig::default().frequency(25.kHz().into());
    let timer = LedcTimerDriver::new(peripherals.ledc.timer0, &config)?;

    let red_pin = peripherals.pins.gpio25;
    let green_pin = peripherals.pins.gpio26;
    let blue_pin = peripherals.pins.gpio27;

    let mut red_channel = LedcDriver::new(peripherals.ledc.channel0, &timer, red_pin)?;
    let mut green_channel = LedcDriver::new(peripherals.ledc.channel1, &timer, green_pin)?;
    let mut blue_channel = LedcDriver::new(peripherals.ledc.channel2, &timer, blue_pin)?;

    let max_duty = red_channel.get_max_duty();

    let mut buf = [0u8; 64];
    let mut current_color = 0;

    loop {
        match socket.recv_from(&mut buf) {
            std::result::Result::Ok((size, _)) => {
                if let std::result::Result::Ok(data) = std::str::from_utf8(&buf[..size]) {
                    if data.starts_with("TOGGLE") {
                        current_color = (current_color + 1) % 6;
                    } else if let std::result::Result::Ok(angle) = u8::from_str(data) {
                        let intensity = (angle as u32 * max_duty) / 180;
                        match current_color {
                            0 => set_color(&mut red_channel, &mut green_channel, &mut blue_channel, intensity, 0, 0)?,
                            1 => set_color(&mut red_channel, &mut green_channel, &mut blue_channel, 0, intensity, 0)?,
                            2 => set_color(&mut red_channel, &mut green_channel, &mut blue_channel, 0, 0, intensity)?,
                            3 => set_color(&mut red_channel, &mut green_channel, &mut blue_channel, intensity, intensity, 0)?,
                            4 => set_color(&mut red_channel, &mut green_channel, &mut blue_channel, intensity, 0, intensity)?,
                            5 => set_color(&mut red_channel, &mut green_channel, &mut blue_channel, 0, intensity, intensity)?,
                            _ => {}
                        }
                    }
                }
            }
            Err(e) => log::error!("Error receiving data: {:?}", e),
        }
    }
}

fn set_color(
    red: &mut LedcDriver<'_>,
    green: &mut LedcDriver<'_>,
    blue: &mut LedcDriver<'_>,
    r: u32,
    g: u32,
    b: u32,
) -> anyhow::Result<()> {
    red.set_duty(r)?;
    green.set_duty(g)?;
    blue.set_duty(b)?;
    Ok(())
}