use lb_rs::text::buffer::Buffer;
use spellbook::Dictionary;

use super::bounds::Words;

pub struct Spelling {
    pub dict: Dictionary,
    pub errors: Words,
}

impl Spelling {
    pub fn new() -> Self {
        let aff = std::fs::read_to_string("/Users/travis/downloads/index.aff").unwrap();
        let dic = std::fs::read_to_string("/Users/travis/downloads/index.dic").unwrap();
        let dict = Dictionary::new(&aff, &dic).unwrap();

        Self { dict, errors: Vec::new() }
    }

    pub fn check(&mut self, words: &Words, buffer: &Buffer) {
        let start = std::time::Instant::now();

        self.errors.clear();
        for &word in words.iter() {
            let word_text = &buffer[word];

            if !word_text.chars().any(|c| c.is_alphabetic()) {
                // words must contain a letter
                continue;
            }
            if word_text.chars().any(|c| c.is_numeric()) {
                // words must not contain a number
                continue;
            }

            if !self.dict.check(&buffer[word]) {
                self.errors.push(word);
            }
        }

        println!("Spelling check took: {:?}", start.elapsed());
    }
}
