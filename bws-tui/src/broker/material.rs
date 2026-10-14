//! Daemon-only secret bytes.
//!
//! [`SecretMaterial`] has no `Clone`, `Debug`, `Display`, or `Serialize` and no
//! public constructor, so it cannot cross the socket or be logged by accident.

use secrecy::{ExposeSecret, SecretBox};
use zeroize::Zeroize;

/// Secret value bytes held only inside daemon code. Zeroized on drop.
pub(crate) struct SecretMaterial(SecretBox<[u8]>);

impl SecretMaterial {
    /// Crate-private constructor. Only vault backends in this module tree call it.
    #[allow(dead_code)]
    pub(super) fn new(bytes: Vec<u8>) -> Self {
        Self(SecretBox::new(bytes.into_boxed_slice()))
    }

    /// Borrow the value bytes for in-daemon use. Callers must not log or frame them.
    #[allow(dead_code)]
    pub(super) fn expose_bytes(&self) -> &[u8] {
        self.0.expose_secret()
    }
}

impl Zeroize for SecretMaterial {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl Drop for SecretMaterial {
    fn drop(&mut self) {
        self.zeroize();
    }
}
