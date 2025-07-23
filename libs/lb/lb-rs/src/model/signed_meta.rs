use super::crypto::ECSigned;
use super::meta::Meta;

pub type SignedMeta = ECSigned<Meta>;

// Impl'd to avoid comparing encrypted
impl PartialEq for SignedMeta {
    fn eq(&self, other: &Self) -> bool {
        self.timestamped_value.value == other.timestamped_value.value
            && self.public_key == other.public_key
    }
}
