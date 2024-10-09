use anyhow::*;
use colored::Colorize;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::ledc::{config::TimerConfig, LedcDriver, LedcTimerDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::FromValueType;
use esp_idf_sys as _;
use std::cmp::PartialEq;

struct TransitionCommands {
    should_transform_red: bool,
    should_transform_green: bool,
    should_transform_blue: bool,
}
enum Color {
    Red,
    Green,
    Blue,
}

#[derive(Debug)]
struct ColorState {
    r: u32,
    g: u32,
    b: u32,
}

impl ColorState {
    pub fn increment(&mut self, color: &Color, target: u32) -> u32 {
        let value = self.get_color_mut(color);
        (*value + 1).min(target)
    }

    pub fn decrement(&mut self, color: &Color, target: u32) -> u32 {
        let value = self.get_color_mut(color);
        (*value - 1).min(target)
    }

    fn get_color(&self, color: &Color) -> &u32 {
        match color {
            Color::Red => &self.r,
            Color::Green => &self.g,
            Color::Blue => &self.b,
        }
    }

    fn get_color_mut(&mut self, color: &Color) -> &mut u32 {
        match color {
            Color::Red => &mut self.r,
            Color::Green => &mut self.g,
            Color::Blue => &mut self.b,
        }
    }

    pub fn transform(&mut self, target: &u32, color: &Color) -> u32 {
        let value = self.get_color_mut(color);
        log::info!("Value for transform is: {}", value);
        if value == target {
            *value
        } else if *value > *target {
            self.decrement(color, *target)
        } else if *value < *target {
            self.increment(color, *target)
        } else {
            *value
        }
    }
    pub fn target_reached(&self, target: &u32, color: &Color) -> bool {
        let value = self.get_color(color);
        *value >= *target
    }

    pub fn compare_to_target(&mut self, target_state: &ColorState) -> TransitionCommands {
        let should_transform_red = !self.target_reached(&target_state.r, &Color::Red);
        let should_transform_green = !self.target_reached(&target_state.g, &Color::Green);
        let should_transform_blue = !self.target_reached(&target_state.b, &Color::Blue);
        TransitionCommands { should_transform_red, should_transform_green, should_transform_blue }
    }
}

impl PartialEq for ColorState {
    fn eq(&self, other: &Self) -> bool {
        self.r == other.r && self.g == other.g && self.b == other.b
    }
}


fn main() {
    // Required to link the ESP-IDF runtime patches.
    esp_idf_svc::sys::link_patches();

    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Starting app!");

    let peripherals = Peripherals::take().unwrap();

    let config = TimerConfig::default().frequency(25.kHz().into());
    let timer = LedcTimerDriver::new(peripherals.ledc.timer0, &config).unwrap();


    let red_pin = peripherals.pins.gpio25;
    let blue_led = peripherals.pins.gpio26;
    let green_led = peripherals.pins.gpio27;

    let mut red_channel = LedcDriver::new(
        peripherals.ledc.channel0,
        &timer,
        red_pin,
    ).unwrap();

    let mut green_channel = LedcDriver::new(peripherals.ledc.channel1, &timer, blue_led).unwrap();
    let mut blue_channel = LedcDriver::new(peripherals.ledc.channel2, &timer, green_led).unwrap();

    let max_duty = red_channel.get_max_duty();

    let color_sequence = vec![
        ColorState { r: 0, g: 0, b: 0 },
        ColorState { r: max_duty, g: 0, b: 0 },  // Red
        ColorState { r: 0, g: max_duty, b: 0 },  // Green
        ColorState { r: 0, g: 0, b: max_duty },  // Blue
        ColorState { r: max_duty, g: max_duty, b: 0 },  // Yellow
        ColorState { r: max_duty, g: 0, b: max_duty },  // Magenta
        ColorState { r: 0, g: max_duty, b: max_duty },  // Cyan
        ColorState { r: max_duty, g: max_duty, b: max_duty },  // White
    ];


    let initial_state = ColorState { r: 1, g: 6, b: 9 };
    let mut current_state = initial_state;
    let mut color_index = 0;

    loop {
        let target_state = &color_sequence[color_index];
        log::info!("Transitioning to color state {}: {:?}", color_index, target_state);

        while current_state != *target_state {
            current_state = transition_color(&mut current_state, &target_state, &mut red_channel, &mut green_channel, &mut blue_channel);
        }
        color_index = (color_index + 1) % color_sequence.len();

        FreeRtos::delay_ms(1);
    }


    fn transition_color(
        previous_state: &mut ColorState,
        target_state: &ColorState,
        red_channel: &mut LedcDriver<'_>,
        green_channel: &mut LedcDriver<'_>,
        blue_channel: &mut LedcDriver<'_>,
    ) -> ColorState {
        log::info!("Transition color: {:?} to: {:?}", previous_state, target_state);

        let TransitionCommands { should_transform_red: should_increase_red, should_transform_green: should_increase_green, should_transform_blue: should_increase_blue } = previous_state.compare_to_target(target_state);
        log::info!("{} {} {} | {} {} | {} {}",
    "Should increase: ".white(),
    "red:".red(),
    if should_increase_red { "true".blue() } else { "false".truecolor(255, 165, 0) },
    "green:".green(),
    if should_increase_green { "true".blue() } else { "false".truecolor(255, 165, 0) },
    "blue:".blue(),
    if should_increase_blue { "true".blue() } else { "false".truecolor(255, 165, 0) }
);

        let transition_state: ColorState = ColorState {
            r: previous_state.transform(&target_state.r, &Color::Red),
            g: previous_state.transform(&target_state.g, &Color::Green),
            b: previous_state.transform(&target_state.b, &Color::Blue),
        };
        log::info!("{}{}{}",transition_state.r, transition_state.g, transition_state.b);
        set_color(red_channel, green_channel, blue_channel, transition_state.r, transition_state.g, transition_state.b).expect("Error setting colors");
        transition_state
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
}