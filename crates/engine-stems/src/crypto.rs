use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyInit, block_padding::Pkcs7};
use anyhow::{Result, anyhow};

// Public format key verified against OfflineAnalyzer and the recorded AAC block.
const KEY: [u8; 16] = [
    0xa3, 0xd5, 0x60, 0x32, 0xf4, 0x50, 0xec, 0xfa, 0x4a, 0xc7, 0x32, 0x86, 0xda, 0x06, 0xd9, 0xe9,
];

pub fn encrypt_sample(sample: &[u8]) -> Vec<u8> {
    ecb::Encryptor::<aes::Aes128>::new(&KEY.into()).encrypt_padded_vec::<Pkcs7>(sample)
}

/// Malformed ciphertext or padding is an error, never silently passed through.
pub fn decrypt_sample(sample: &[u8]) -> Result<Vec<u8>> {
    ecb::Decryptor::<aes::Aes128>::new(&KEY.into())
        .decrypt_padded_vec::<Pkcs7>(sample)
        .map_err(|_| anyhow!("sample has invalid AES block length or PKCS#7 padding"))
}
