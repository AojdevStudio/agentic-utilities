//! Daemon-only secret bytes.
//!
//! [`SecretMaterial`] has no `Clone`, `Debug`, `Display`, or `Serialize` and no
//! public constructor, so it cannot cross the socket or be logged by accident.

use secrecy::{ExposeSecret, SecretBox};
use zeroize::{Zeroize, Zeroizing};

/// Secret value bytes held only inside daemon code. Zeroized on drop.
pub(crate) struct SecretMaterial(SecretBox<[u8]>);

impl SecretMaterial {
    /// Constructor private to the broker tree: vault backends and daemon value input.
    /// The bytes are copied into an exact-size allocation and the input is
    /// wiped, because shrinking a `Vec` can move it and free the old copy unwiped.
    pub(super) fn new(bytes: Vec<u8>) -> Self {
        let bytes = Zeroizing::new(bytes);
        let mut exact = vec![0u8; bytes.len()].into_boxed_slice();
        exact.copy_from_slice(&bytes);
        Self(SecretBox::new(exact))
    }

    /// Borrow the value bytes for in-daemon use. Callers must not log or frame them.
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
