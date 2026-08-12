//! Shared HTTPS client construction.
//!
//! RaViChara uses rustls instead of the Windows Schannel client so a broken
//! process-level Schannel credential state cannot disable every online model
//! and TTS provider. Trust still comes from the operating-system root store;
//! no certificate validation is bypassed.

pub fn builder() -> reqwest::ClientBuilder {
    // Explicit installation avoids depending on rustls' compile-time provider
    // inference when reqwest and this crate select features independently.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut builder = reqwest::Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false);
    let certificates = platform_root_certificates();
    if certificates.is_empty() {
        tracing::warn!(
            "no platform TLS root certificates were loaded; HTTPS providers may be unavailable"
        );
    }
    for certificate in certificates {
        builder = builder.add_root_certificate(certificate);
    }
    builder
}

#[cfg(windows)]
fn platform_root_certificates() -> Vec<reqwest::Certificate> {
    use std::collections::HashSet;
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::Cryptography::{
        CertCloseStore, CertEnumCertificatesInStore, CertOpenStore,
        CERT_STORE_OPEN_EXISTING_FLAG, CERT_STORE_PROV_SYSTEM_W,
        CERT_STORE_READONLY_FLAG, CERT_SYSTEM_STORE_CURRENT_USER,
        CERT_SYSTEM_STORE_LOCAL_MACHINE,
    };

    let root_name = std::ffi::OsStr::new("ROOT")
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut encoded_roots = HashSet::<Vec<u8>>::new();
    for location in [
        CERT_SYSTEM_STORE_CURRENT_USER,
        CERT_SYSTEM_STORE_LOCAL_MACHINE,
    ] {
        let flags = location
            | CERT_STORE_OPEN_EXISTING_FLAG
            | CERT_STORE_READONLY_FLAG;
        // SAFETY: the UTF-16 store name remains alive for the entire call. The
        // returned store is closed below, and CertEnumCertificatesInStore owns
        // and releases each previous context as enumeration advances.
        let store = unsafe {
            CertOpenStore(
                CERT_STORE_PROV_SYSTEM_W,
                0,
                0,
                flags,
                root_name.as_ptr().cast::<c_void>(),
            )
        };
        if store.is_null() {
            continue;
        }
        let mut previous = std::ptr::null();
        loop {
            // SAFETY: `store` is valid and `previous` is either null or the
            // context returned by the preceding enumeration call.
            let context = unsafe { CertEnumCertificatesInStore(store, previous) };
            if context.is_null() {
                break;
            }
            // SAFETY: Windows guarantees the encoded certificate buffer is
            // valid for the lifetime of this context; it is copied immediately.
            let encoded = unsafe {
                let context_ref = &*context;
                if context_ref.pbCertEncoded.is_null()
                    || context_ref.cbCertEncoded == 0
                {
                    previous = context;
                    continue;
                }
                std::slice::from_raw_parts(
                    context_ref.pbCertEncoded,
                    context_ref.cbCertEncoded as usize,
                )
                .to_vec()
            };
            encoded_roots.insert(encoded);
            previous = context;
        }
        // SAFETY: `store` was returned by CertOpenStore and no certificate
        // contexts remain owned by this function after enumeration terminates.
        unsafe {
            CertCloseStore(store, 0);
        }
    }
    encoded_roots
        .into_iter()
        .filter_map(|encoded| reqwest::Certificate::from_der(&encoded).ok())
        .collect()
}

#[cfg(not(windows))]
fn platform_root_certificates() -> Vec<reqwest::Certificate> {
    const BUNDLES: &[&str] = &[
        "/etc/ssl/certs/ca-certificates.crt",
        "/etc/pki/tls/certs/ca-bundle.crt",
        "/etc/ssl/ca-bundle.pem",
        "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
    ];
    for path in BUNDLES {
        let Ok(contents) = std::fs::read(path) else {
            continue;
        };
        if let Ok(certificates) = reqwest::Certificate::from_pem_bundle(&contents) {
            if !certificates.is_empty() {
                return certificates;
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn platform_trust_store_is_available() {
        assert!(!super::platform_root_certificates().is_empty());
    }
}
