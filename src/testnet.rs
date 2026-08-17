pub(crate) const LOGOS_TESTNET_PRESET: &str = "logos.test";
pub(crate) const LOGOS_TESTNET_CHANNEL_ID: &str =
    "0101010101010101010101010101010101010101010101010101010101010101";
pub(crate) const LOCAL_BEDROCK_ENDPOINT: &str = "http://127.0.0.1:8080/";
pub(crate) const LOCAL_INDEXER_ENDPOINT: &str = "http://127.0.0.1:8779/";
pub(crate) const LEZ_TESTNET_SEQUENCER_ENDPOINT: &str = "https://testnet.lez.logos.co/";

/// Bedrock's libp2p network peers from the pinned blockchain module's live
/// Testnet profile. These are not the Blend service locators published in the
/// Testnet genesis providers list (ports 3400-3402/50002). Keep this list in
/// sync with the module profile: stale bootstrap addresses leave Bedrock in
/// `Bootstrapping` even when its HTTP endpoint is reachable.
pub(crate) const LOGOS_TESTNET_BOOTSTRAP_PEERS: &[&str] = &[
    "/ip4/209.38.241.182/udp/3001/quic-v1",
    "/ip4/209.38.241.182/udp/3000/quic-v1",
    "/ip4/209.38.241.182/udp/3002/quic-v1",
    "/ip4/209.38.241.182/udp/3003/quic-v1",
];

#[cfg(test)]
mod tests {
    use super::LOGOS_TESTNET_BOOTSTRAP_PEERS;

    #[test]
    fn testnet_bootstrap_peers_are_bedrock_network_peers() {
        assert_eq!(LOGOS_TESTNET_BOOTSTRAP_PEERS.len(), 4);
        for peer in LOGOS_TESTNET_BOOTSTRAP_PEERS {
            assert!(peer.starts_with("/ip4/209.38.241.182/udp/"));
            assert!(peer.ends_with("/quic-v1"));
            assert!(!peer.contains("/p2p/"));
            assert!(!peer.contains("/udp/340"));
            assert!(!peer.contains("/udp/50002"));
        }

        assert!(
            LOGOS_TESTNET_BOOTSTRAP_PEERS
                .iter()
                .any(|peer| peer.contains("/udp/3000/"))
        );
        assert!(
            LOGOS_TESTNET_BOOTSTRAP_PEERS
                .iter()
                .any(|peer| peer.contains("/udp/3001/"))
        );
        assert!(
            LOGOS_TESTNET_BOOTSTRAP_PEERS
                .iter()
                .any(|peer| peer.contains("/udp/3002/"))
        );
        assert!(
            LOGOS_TESTNET_BOOTSTRAP_PEERS
                .iter()
                .any(|peer| peer.contains("/udp/3003/"))
        );
    }
}
