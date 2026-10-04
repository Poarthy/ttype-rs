use std::collections::BTreeMap;

use thiserror::Error;

use crate::domain::{GenerateOptions, TextMode};

pub const MIN_TARGET_RUNES: usize = 4096;

const NUMBER_CHANCE: f64 = 0.1;
const PUNCTUATION_CHANCE: f64 = 0.14;
const COMMA_SHARE: f64 = 0.55;

#[derive(Debug, Error)]
pub enum TextError {
    #[error("unsupported mode {0:?}")]
    UnsupportedMode(TextMode),
    #[error("mode {0:?} has no practice items")]
    EmptyMode(TextMode),
}

pub trait TextSource {
    fn generate(&self, options: &GenerateOptions) -> Result<String, TextError>;

    fn word_lines(&self, _: &GenerateOptions) -> Option<Vec<usize>> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct StaticText {
    text: String,
    word_lines: Vec<usize>,
}

impl StaticText {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            word_lines: Vec::new(),
        }
    }

    pub fn with_word_lines(text: impl Into<String>, word_lines: Vec<usize>) -> Self {
        Self {
            text: text.into(),
            word_lines,
        }
    }
}

impl TextSource for StaticText {
    fn generate(&self, _: &GenerateOptions) -> Result<String, TextError> {
        Ok(self.text.clone())
    }

    fn word_lines(&self, _: &GenerateOptions) -> Option<Vec<usize>> {
        (!self.word_lines.is_empty()).then(|| self.word_lines.clone())
    }
}

#[derive(Clone, Debug, Default)]
pub struct TextProvider {
    items: BTreeMap<TextMode, Vec<String>>,
}

impl TextProvider {
    pub fn new(items: BTreeMap<TextMode, Vec<String>>) -> Self {
        Self { items }
    }

    pub fn items(&self) -> &BTreeMap<TextMode, Vec<String>> {
        &self.items
    }

    pub fn with_mode_items(mut self, mode: TextMode, items: Vec<String>) -> Self {
        self.items.insert(mode, items);
        self
    }

    pub fn generate(&self, options: &GenerateOptions) -> Result<String, TextError> {
        <Self as TextSource>::generate(self, options)
    }
}

impl TextSource for TextProvider {
    fn generate(&self, options: &GenerateOptions) -> Result<String, TextError> {
        let items = self
            .items
            .get(&options.mode)
            .ok_or(TextError::UnsupportedMode(options.mode))?;
        if items.is_empty() {
            return Err(TextError::EmptyMode(options.mode));
        }

        let mut random = GoPcg::new(options.seed as u64, (options.seed >> 32) as u64);
        let mut sampled = sample(items, options.word_limit, &mut random);
        if options.numbers && matches!(options.mode, TextMode::Words) {
            apply_numbers(&mut sampled, &mut random);
        }
        Ok(join_items(
            &sampled,
            options.punctuation && matches!(options.mode, TextMode::Words),
            &mut random,
        ))
    }
}

fn sample(items: &[String], limit: usize, random: &mut GoPcg) -> Vec<String> {
    if limit > 0 {
        return (0..limit)
            .map(|_| items[random.int_n(items.len())].clone())
            .collect();
    }

    let mut length = 0;
    let mut sampled = Vec::new();
    while length < MIN_TARGET_RUNES {
        let item = items[random.int_n(items.len())].clone();
        if !sampled.is_empty() {
            length += 1;
        }
        // The Go provider uses len(string), which is UTF-8 byte length.
        length += item.len();
        sampled.push(item);
    }
    sampled
}

fn apply_numbers(items: &mut [String], random: &mut GoPcg) {
    for item in items {
        if random.float64() < NUMBER_CHANCE {
            *item = (random.int_n(9999) + 1).to_string();
        }
    }
}

fn join_items(items: &[String], punctuation: bool, random: &mut GoPcg) -> String {
    let Some((first, rest)) = items.split_first() else {
        return String::new();
    };
    let mut target = first.clone();
    for item in rest {
        let separator = if punctuation && random.float64() < PUNCTUATION_CHANCE {
            if random.float64() < COMMA_SHARE {
                ", "
            } else {
                ". "
            }
        } else {
            " "
        };
        target.push_str(separator);
        target.push_str(item);
    }
    target
}

/// Go's math/rand/v2 PCG-DXSM source, reproduced so seeded targets retain
/// the reference implementation's deterministic stream without a runtime RNG.
#[derive(Clone, Debug)]
struct GoPcg {
    state: u128,
}

impl GoPcg {
    const MULTIPLIER: u128 =
        (2_549_297_995_355_413_924_u128 << 64) | 4_865_540_595_714_422_341_u128;
    const INCREMENT: u128 = (6_364_136_223_846_793_005_u128 << 64) | 1_442_695_040_888_963_407_u128;
    const CHEAP_MULTIPLIER: u64 = 0xda94_2042_e4dd_58b5;

    fn new(high: u64, low: u64) -> Self {
        Self {
            state: (u128::from(high) << 64) | u128::from(low),
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::INCREMENT);
        let high = (self.state >> 64) as u64;
        let low = self.state as u64;
        let mixed = (high ^ (high >> 32)).wrapping_mul(Self::CHEAP_MULTIPLIER);
        let mixed = mixed ^ (mixed >> 48);
        mixed.wrapping_mul(low | 1)
    }

    fn int_n(&mut self, upper_bound: usize) -> usize {
        let upper = upper_bound as u64;
        let mut value = self.next_u64();
        let mut product = u128::from(value) * u128::from(upper);
        let mut low = product as u64;
        if low < upper {
            let threshold = upper.wrapping_neg() % upper;
            while low < threshold {
                value = self.next_u64();
                product = u128::from(value) * u128::from(upper);
                low = product as u64;
            }
        }
        (product >> 64) as usize
    }

    fn float64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
    }
}
