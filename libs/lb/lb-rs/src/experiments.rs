use std::hash::{DefaultHasher, Hash, Hasher};
use strum::VariantArray;
use strum_macros::VariantArray;

#[derive(VariantArray, Clone, Copy)]
pub enum WelcomeDoc {
    NoWelcomeDoc,
    OldWelcomeDoc,
    HypeWelcomeDoc,
    FramgentedArchetypes,
}

pub fn cohort<E: VariantArray + Copy>(username: &str) -> E {
    let mut hasher = DefaultHasher::new();
    username.hash(&mut hasher);
    let hash = hasher.finish();

    let variant_id = (hash % E::VARIANTS.len() as u64) as usize;

    E::VARIANTS[variant_id]
}
