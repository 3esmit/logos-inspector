/// Source-level signature declared by the versioned Storage download API.
pub(crate) const STORAGE_DOWNLOAD_V2_METHOD: &str = "downloadToUrlV2";
pub(crate) const STORAGE_DOWNLOAD_V2_METHOD_SIGNATURE: &str =
    "downloadToUrlV2(QString,QString,bool,int,QString,int)";

/// Universal Logos module glue represents the two integer parameters as
/// `QVariant` in `module-info`, while dispatching the same six-argument C++
/// method. Accept this exact generated shape alongside the source-level one;
/// the caller still passes JSON numbers and the Storage V2 protocol validates
/// their bounds before starting a download.
pub(crate) const STORAGE_DOWNLOAD_V2_UNIVERSAL_METHOD_SIGNATURE: &str =
    "downloadToUrlV2(QString,QString,bool,QVariant,QString,QVariant)";

pub(crate) const STORAGE_DOWNLOAD_V2_METHOD_SIGNATURES: [&str; 2] = [
    STORAGE_DOWNLOAD_V2_METHOD_SIGNATURE,
    STORAGE_DOWNLOAD_V2_UNIVERSAL_METHOD_SIGNATURE,
];

#[must_use]
pub(crate) fn is_storage_download_v2_method_signature(signature: &str) -> bool {
    STORAGE_DOWNLOAD_V2_METHOD_SIGNATURES.contains(&signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_documented_and_universal_v2_metadata_shapes() {
        for signature in STORAGE_DOWNLOAD_V2_METHOD_SIGNATURES {
            assert!(is_storage_download_v2_method_signature(signature));
        }
        assert!(!is_storage_download_v2_method_signature(
            "downloadToUrlV2(QString,QString,bool,QVariant,QString)"
        ));
        assert!(!is_storage_download_v2_method_signature(
            "downloadToUrl(QString,QString,bool,QVariant,QString,QVariant)"
        ));
    }
}
