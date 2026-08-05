#[cfg(test)]
use std::sync::{Mutex, OnceLock};

const SERVICE_NAME: &str = "com.storymemory.desktop";
const ACCOUNT_NAME: &str = "openai-api-key";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialStatus {
    pub configured: bool,
}

#[cfg(target_os = "macos")]
fn keychain_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME)
        .map_err(|error| format!("Der Betriebssystem-Schlüsselbund ist nicht verfügbar: {error}"))
}

#[cfg(target_os = "macos")]
pub fn set_openai_api_key(api_key: &str) -> Result<CredentialStatus, String> {
    keychain_entry()?.set_password(api_key).map_err(|error| {
        format!("Der API-Schlüssel konnte nicht sicher gespeichert werden: {error}")
    })?;
    Ok(CredentialStatus { configured: true })
}

#[cfg(not(target_os = "macos"))]
pub fn set_openai_api_key(_api_key: &str) -> Result<CredentialStatus, String> {
    Err("Der Betriebssystem-Schlüsselbund ist auf diesem System nicht verfügbar.".into())
}

#[cfg(target_os = "macos")]
pub fn has_openai_api_key() -> Result<CredentialStatus, String> {
    let configured = keychain_entry()?.get_password().is_ok();
    Ok(CredentialStatus { configured })
}

#[cfg(not(target_os = "macos"))]
pub fn has_openai_api_key() -> Result<CredentialStatus, String> {
    Ok(CredentialStatus { configured: false })
}

#[cfg(target_os = "macos")]
pub fn read_openai_api_key() -> Result<String, String> {
    keychain_entry()?.get_password().map_err(|error| {
        format!("Kein API-Schlüssel im Betriebssystem-Schlüsselbund verfügbar: {error}")
    })
}

#[cfg(not(target_os = "macos"))]
pub fn read_openai_api_key() -> Result<String, String> {
    Err("Der Betriebssystem-Schlüsselbund ist auf diesem System nicht verfügbar.".into())
}

#[cfg(target_os = "macos")]
pub fn delete_openai_api_key() -> Result<CredentialStatus, String> {
    let entry = keychain_entry()?;
    match entry.delete_credential() {
        Ok(()) => Ok(CredentialStatus { configured: false }),
        Err(keyring::Error::NoEntry) => Ok(CredentialStatus { configured: false }),
        Err(error) => Err(format!(
            "Der API-Schlüssel konnte nicht entfernt werden: {error}"
        )),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn delete_openai_api_key() -> Result<CredentialStatus, String> {
    Ok(CredentialStatus { configured: false })
}

#[cfg(test)]
pub mod test_adapter {
    use super::*;

    static FAKE_KEY: OnceLock<Mutex<Option<String>>> = OnceLock::new();

    fn store() -> &'static Mutex<Option<String>> {
        FAKE_KEY.get_or_init(|| Mutex::new(None))
    }

    pub fn set(value: &str) -> CredentialStatus {
        *store().lock().expect("fake credential store") = Some(value.to_owned());
        CredentialStatus { configured: true }
    }

    pub fn get() -> Option<String> {
        store().lock().expect("fake credential store").clone()
    }

    pub fn clear() {
        *store().lock().expect("fake credential store") = None;
    }

    #[test]
    fn fake_credentials_are_never_part_of_a_status_response() {
        clear();
        let status = set("fake-test-key");
        assert!(status.configured);
        assert_eq!(get().as_deref(), Some("fake-test-key"));
        clear();
        assert_eq!(get(), None);
    }
}
