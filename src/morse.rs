use embedded_hal::digital::OutputPin;
#[cfg(not(feature = "precision_delay"))]
use esp_idf_hal::delay::Delay;
use log::*;

use crate::param::KeyerParam;

pub enum MorseCode {
    Di,
    Dah,
}

pub struct Morse<PINDI, PINDAH> {
    pin_di: PINDI,
    pin_dah: PINDAH,
    ratio: u32,
    letter_space: u32,
    word_space: u32,
    tick: u32,
    reverse: bool,
    morse_table: Vec<(char, u8, u8)>,
}

impl<PINDI, PINDAH> Morse<PINDI, PINDAH>
where
    PINDI: OutputPin,
    PINDAH: OutputPin,
{
    const MSPERWPM: u32 = 1200; /* PARIS = 50 tick */

    pub fn new(mut pin_di: PINDI, mut pin_dah: PINDAH) -> Self {
        let _ = pin_di.set_low();
        let _ = pin_dah.set_low();
        Self {
            pin_di,
            pin_dah,
            ratio: 3,
            word_space: 7,
            letter_space: 3,
            tick: Self::MSPERWPM / 20,
            reverse: false,
            morse_table: vec![
                ('0', 5, 0x1f), // '0' : -----
                ('1', 5, 0x1e), // '1' : .----
                ('2', 5, 0x1c), // '2' : ..---
                ('3', 5, 0x18), // '3' : ...--
                ('4', 5, 0x10), // '4' : ....-
                ('5', 5, 0x00), // '5' : .....
                ('6', 5, 0x01), // '6' : -....
                ('7', 5, 0x03), // '7' : --...
                ('8', 5, 0x07), // '8' : ---..
                ('9', 5, 0x0f), // '9' : ----.
                ('A', 2, 0x02), // 'A' : .-
                ('B', 4, 0x01), // 'B' : -...
                ('C', 4, 0x05), // 'C' : -.-.
                ('D', 3, 0x01), // 'D' : -..
                ('E', 1, 0x00), // 'E' : .
                ('F', 4, 0x04), // 'F' : ..-.
                ('G', 3, 0x03), // 'G' : --.
                ('H', 4, 0x00), // 'H' : ....
                ('I', 2, 0x00), // 'I' : ..
                ('J', 4, 0x0e), // 'J' : .---
                ('K', 3, 0x05), // 'K' : -.-
                ('L', 4, 0x02), // 'L' : .-..
                ('M', 2, 0x03), // 'M' : --
                ('N', 2, 0x01), // 'N' : -.
                ('O', 3, 0x07), // 'O' : ---
                ('P', 4, 0x06), // 'P' : .--.
                ('Q', 4, 0x0b), // 'Q' : --.-
                ('R', 3, 0x02), // 'R' : .-.
                ('S', 3, 0x00), // 'S' : ...
                ('T', 1, 0x01), // 'T' : -
                ('U', 3, 0x04), // 'U' : ..-
                ('V', 4, 0x08), // 'V' : ...-
                ('W', 3, 0x06), // 'W' : .--
                ('X', 4, 0x09), // 'X' : -..-
                ('Y', 4, 0x0d), // 'Y' : -.--
                ('Z', 4, 0x03), // 'Z' : --..
                ('/', 5, 0x09), // '/' : -..-.
                ('?', 6, 0x0c), // '?' : ..--..
                ('.', 6, 0x2a), // '.' : .-.-.-
                (',', 6, 0x33), // ',' : --..--
                ('=', 5, 0x11), // '=' : -...-
                ('!', 6, 0x35), // '!' : -.-.--
                ('+', 5, 0x0a), // '+' : .-.-.
                ('-', 6, 0x21), // '-' : -....-
            ],
        }
    }

    pub fn set_wpm(&mut self, wpm: u32) {
        self.tick = Self::MSPERWPM / wpm
    }

    pub fn set_ratio(&mut self, ratio: u32) {
        self.ratio = ratio
    }

    pub fn set_letter_space(&mut self, ls: u32) {
        self.letter_space = ls
    }

    pub fn set_word_space(&mut self, ws: u32) {
        self.word_space = ws
    }

    pub fn normal(&mut self) {
        self.reverse = false;
    }

    pub fn reverse(&mut self) {
        self.reverse = true;
    }

    fn wait(&self, ms: u32) {
        #[cfg(feature = "precision_delay")]
        {
            use esp_idf_sys::{xPortGetTickRateHz, xTaskDelayUntil, xTaskGetTickCount, TickType_t};
            let ticktowait: TickType_t = ms * unsafe { xPortGetTickRateHz() } / 1000;
            let mut lastwake: TickType_t = unsafe { xTaskGetTickCount() };
            unsafe {
                xTaskDelayUntil(&mut lastwake, ticktowait);
            }
        }
        #[cfg(not(feature = "precision_delay"))]
        Delay::delay_ms(ms);
    }

    fn assert(&mut self, mut pin: MorseCode, tick: u32) {
        if self.reverse {
            match pin {
                MorseCode::Dah => pin = MorseCode::Di,
                MorseCode::Di => pin = MorseCode::Dah,
            }
        }
        match pin {
            MorseCode::Di => {
                let _ = self.pin_di.set_high();
                self.wait(tick);
                let _ = self.pin_di.set_low();
            }
            MorseCode::Dah => {
                let _ = self.pin_dah.set_high();
                self.wait(tick);
                let _ = self.pin_dah.set_low();
            }
        }
    }

    pub fn play_keyout(&mut self, c: char) {
        let is_di = |x: u8| (x & 1) == 0;
        let c = c.to_ascii_uppercase();
        if let Some((_, clen, mut code)) = self.morse_table.iter().find(|x| x.0 == c) {
            for _ in 0..*clen {
                if is_di(code) {
                    self.assert(MorseCode::Dah, self.tick);
                } else {
                    self.assert(MorseCode::Dah, self.tick * self.ratio);
                }
                self.wait(self.tick);
                code >>= 1;
            }
        }
    }

    pub fn play_paddle(&mut self, c: char) {
        let c = c.to_ascii_uppercase();
        if let Some((_, clen, mut code)) = self.morse_table.iter().find(|x| x.0 == c) {
            let is_di = |x: u8| (x & 1) == 0;
            for _ in 0..*clen {
                if is_di(code) {
                    self.assert(MorseCode::Di, self.tick);
                } else {
                    self.assert(MorseCode::Dah, self.tick * self.ratio);
                }
                code >>= 1;
                self.wait(self.tick);
            }
        }
    }

    pub fn play(&mut self, paddle: bool, message: &String) {
        info!("{message} ");
        for c in message.chars() {
            match c {
                ' ' => {
                    self.wait(self.tick * (self.word_space));
                }

                '#' => {
                    self.wait(1000);
                }

                _ => {
                    if paddle {
                        self.play_paddle(c);
                    } else {
                        self.play_keyout(c);
                    }
                    self.wait(self.tick * (self.letter_space));
                }
            }
        }
    }

    pub fn interp(&mut self, param: &KeyerParam) {
        if let Some(s) = param.wpm {
            self.set_wpm(s);
        }
        if let Some(r) = param.ratio {
            self.set_ratio(r);
        }
        if let Some(s) = param.letter_space {
            self.set_letter_space(s);
        }
        if let Some(s) = param.word_space {
            self.set_word_space(s);
        }
        if let Some(r) = param.reverse {
            if r {
                self.reverse();
            } else {
                self.normal();
            }
        }
        if param.to_paddle.is_some() {
            self.play(true, param.to_paddle.as_ref().unwrap());
        }
        if param.to_straight.is_some() {
            self.normal();
            self.play(false, param.to_straight.as_ref().unwrap());
        }
    }
}
