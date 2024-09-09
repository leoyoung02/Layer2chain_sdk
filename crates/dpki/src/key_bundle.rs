#![allow(warnings)]
use lib3h_sodium::{kx, secbuf::SecBuf, sign, *};

use crate::{
    keypair::*,
    password_encryption::{self, EncryptedData, PwHashConfig},
    seed::{Seed, SeedType},
    utils, SEED_SIZE,
};
use holochain_core_types::{agent::Base32, error::HcResult};
use serde_json::json;
use std::str;

use serde_derive::{Deserialize, Serialize};

/// Struct holding all the keys generated by a seed
pub struct KeyBundle {
    pub sign_keys: SigningKeyPair,
    pub enc_keys: EncryptingKeyPair,
}

#[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_DPKI)]
impl KeyBundle {
    /// create a new KeyBundle
    pub fn new(sign_keys: SigningKeyPair, enc_keys: EncryptingKeyPair) -> HcResult<Self> {
        Ok(KeyBundle {
            sign_keys,
            enc_keys,
        })
    }

    /// Derive the keys from a Seed
    pub fn new_from_seed(seed: &mut Seed) -> HcResult<Self> {
        Ok(KeyBundle {
            sign_keys: SigningKeyPair::new_from_seed(&mut seed.buf)?,
            enc_keys: EncryptingKeyPair::new_from_seed(&mut seed.buf)?,
        })
    }

    /// Derive the keys from a 32 bytes seed buffer
    /// @param {SecBuf} seed - the seed buffer
    /// @param {SeedType} seed_type - seed type of the buffer
    pub fn new_from_seed_buf(seed_buf: &mut SecBuf) -> HcResult<Self> {
        assert_eq!(seed_buf.len(), SEED_SIZE);
        Ok(KeyBundle {
            sign_keys: SigningKeyPair::new_from_seed(seed_buf)?,
            enc_keys: EncryptingKeyPair::new_from_seed(seed_buf)?,
        })
    }

    /// get the identifier key
    pub fn get_id(&self) -> Base32 {
        self.sign_keys.public.clone()
    }

    /// sign some arbitrary data with the signing private key
    /// @param {SecBuf} data - the data to sign
    /// @return {SecBuf} signature - Empty Buf to be filled with the signature
    pub fn sign(&mut self, data: &mut SecBuf) -> HcResult<SecBuf> {
        self.sign_keys.sign(data)
    }

    pub fn encrypt(&mut self, data: &mut SecBuf) -> HcResult<SecBuf> {
        let mut encrypted_data = SecBuf::with_insecure(
            data.len() + lib3h_sodium::aead::ABYTES + lib3h_sodium::aead::NONCEBYTES,
        );
        self.enc_keys.encrypt(data, &mut encrypted_data);
        Ok(encrypted_data)
    }

    pub fn decrypt(&mut self, cipher: &mut SecBuf) -> HcResult<SecBuf> {
        let mut decrypted_data = SecBuf::with_insecure(
            (cipher.len() - lib3h_sodium::aead::NONCEBYTES) + lib3h_sodium::aead::ABYTES,
        );
        self.enc_keys.decrypt(cipher, &mut decrypted_data);
        Ok(decrypted_data)
    }

    /// verify data that was signed with our private signing key
    /// @param {SecBuf} data buffer to verify
    /// @param {SecBuf} signature candidate for that data buffer
    /// @return true if verification succeeded
    pub fn verify(&mut self, data: &mut SecBuf, signature: &mut SecBuf) -> bool {
        self.sign_keys.verify(data, signature)
    }

    ///
    pub fn is_same(&mut self, other: &mut KeyBundle) -> bool {
        self.sign_keys.is_same(&mut other.sign_keys) && self.enc_keys.is_same(&mut other.enc_keys)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{keypair::*, utils::generate_random_seed_buf, SIGNATURE_SIZE};
    use lib3h_sodium::pwhash;

    pub(crate) const TEST_CONFIG: Option<PwHashConfig> = Some(PwHashConfig(
        pwhash::OPSLIMIT_INTERACTIVE,
        pwhash::MEMLIMIT_INTERACTIVE,
        pwhash::ALG_ARGON2ID13,
    ));

    fn test_generate_random_bundle() -> KeyBundle {
        let mut seed = generate_random_seed_buf();
        KeyBundle::new_from_seed_buf(&mut seed).unwrap()
    }

    #[test]
    fn it_should_create_keybundle_from_pairs() {
        let mut seed = generate_random_seed_buf();
        let sign_keys = SigningKeyPair::new_from_seed(&mut seed).unwrap();
        let enc_keys = EncryptingKeyPair::new_from_seed(&mut seed).unwrap();
        let result = KeyBundle::new(sign_keys, enc_keys);
        assert!(result.is_ok());
        let bundle = result.unwrap();
        assert_eq!(64, bundle.sign_keys.private.len());
        assert_eq!(32, bundle.enc_keys.private.len());
    }

    #[test]
    fn it_should_create_keybundle_from_seed() {
        let bundle = test_generate_random_bundle();
        assert_eq!(64, bundle.sign_keys.private.len());
        assert_eq!(32, bundle.enc_keys.private.len());

        let id = bundle.get_id();
        println!("id: {:?}", id);
        assert_ne!(0, id.len());
    }

    #[test]
    fn keybundle_should_sign_message_and_verify() {
        let mut bundle = test_generate_random_bundle();

        // Create random data
        let mut message = SecBuf::with_insecure(16);
        message.randomize();

        // sign it
        let mut signature = bundle.sign(&mut message).unwrap();
        // authentify signature
        let succeeded = bundle.verify(&mut message, &mut signature);
        assert!(succeeded);

        // Create random data
        let mut random_signature = SecBuf::with_insecure(SIGNATURE_SIZE);
        random_signature.randomize();
        // authentify random signature
        let succeeded = bundle.verify(&mut message, &mut random_signature);
        assert!(!succeeded);

        // Randomize data again
        message.randomize();
        let succeeded = bundle.verify(&mut message, &mut signature);
        assert!(!succeeded);
    }

    #[test]

    fn keybundle_should_encrypt_and_decrypt() {
        let mut bundle = test_generate_random_bundle();

        // Create random data
        let mut message = SecBuf::with_insecure(16);
        message.randomize();
        //encrypt it
        let mut encrypted_message = bundle.encrypt(&mut message.clone()).unwrap();

        //decrypted same message
        let mut decrypted_message = bundle.decrypt(&mut encrypted_message).unwrap();

        //read read_lock
        let encrypted_read_lock = encrypted_message.read_lock();

        //read write_lock
        let decrypted_read_lock = decrypted_message.read_lock();

        let message_read_lock = message.read_lock();

        //check if decrypted message equals original message
        assert_eq!(message_read_lock[0..16], decrypted_read_lock[0..16])
    }
}