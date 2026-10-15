//! The provider's field rules, checked before any lookup, prompt, or SDK
//! call. The pinned `bitwarden-sm` 3.0.0 validates key, value, and note
//! lengths in characters and copies a refused field into its validation
//! error and debug log, so nothing it would refuse may reach it. Refusal
//! text never repeats a value.

use super::material::SecretMaterial;
use super::operation::{Operation, Refusal};
use zeroize::Zeroizing;

/// Largest value the daemon reads from a pipe or a prompt, in bytes.
pub(crate) const MAX_VALUE: usize = 64 * 1024;
const MAX_KEY_CHARS: usize = 500;
const MAX_NOTE_CHARS: usize = 7_000;
const MAX_VALUE_CHARS: usize = 25_000;

/// 1 to 500 characters that `str::trim`, which the provider applies, leaves
/// unchanged, so a write never lands under another name.
pub(crate) fn check_key(key: &str) -> Result<(), Refusal> {
    let length = key.chars().count();
    if !(1..=MAX_KEY_CHARS).contains(&length) || key.trim() != key {
        return Err(Refusal::InvalidKey { key: key.into() });
    }
    Ok(())
}

/// At most 7000 characters. Empty is allowed, only whitespace is not.
pub(crate) fn check_note(note: &str) -> Result<(), Refusal> {
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(invalid("the note is longer than 7000 characters"));
    }
    if !note.is_empty() && note.trim().is_empty() {
        return Err(invalid("the note is only whitespace"));
    }
    Ok(())
}

/// A new key or note an operation proposes, before any lookup, audit line,
/// or prompt.
pub(crate) fn check_proposed(op: &Operation) -> Result<(), Refusal> {
    let (key, note) = match op {
        Operation::Add { key, note, .. } => (Some(key), note),
        Operation::Edit { new_key, note, .. } => (new_key.as_ref(), note),
        _ => return Ok(()),
    };
    key.map(|key| check_key(key)).transpose()?;
    note.as_deref().map(check_note).transpose()?;
    Ok(())
}

/// UTF-8 text of 1 to 25000 characters and at most 64 KiB.
pub(crate) fn check_value(value: &[u8]) -> Result<(), Refusal> {
    if value.is_empty() {
        return Err(invalid("the value is empty"));
    }
    if value.len() > MAX_VALUE {
        return Err(invalid("the value is larger than 64 KiB"));
    }
    let Ok(text) = std::str::from_utf8(value) else {
        return Err(invalid("the value is not UTF-8 text"));
    };
    if text.chars().count() > MAX_VALUE_CHARS {
        return Err(invalid("the value is longer than 25000 characters"));
    }
    Ok(())
}

/// Check a value as soon as it is read, then keep it as daemon material.
pub(crate) fn checked_value(mut value: Zeroizing<Vec<u8>>) -> Result<SecretMaterial, Refusal> {
    check_value(&value)?;
    Ok(SecretMaterial::new(std::mem::take(&mut *value)))
}

pub(crate) fn invalid(reason: &str) -> Refusal {
    Refusal::InvalidValue {
        reason: reason.into(),
    }
}
