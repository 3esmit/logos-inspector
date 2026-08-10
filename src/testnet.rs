pub(crate) const LOGOS_TESTNET_PRESET: &str = "logos.test";
pub(crate) const LOGOS_TESTNET_CHANNEL_ID: &str =
    "0101010101010101010101010101010101010101010101010101010101010101";
pub(crate) const LOCAL_BEDROCK_ENDPOINT: &str = "http://127.0.0.1:8080/";
pub(crate) const LOCAL_INDEXER_ENDPOINT: &str = "http://127.0.0.1:8779/";
pub(crate) const LEZ_TESTNET_SEQUENCER_ENDPOINT: &str = "https://testnet.lez.logos.co/";

/// Bedrock's libp2p network peers. These are not the Blend service locators
/// published in the Testnet genesis providers list (ports 3400-3402/50002).
/// Keep the peer IDs: the blockchain module uses them to seed initial block
/// download, while Blend locators cannot be dialed as Bedrock peers.
pub(crate) const LOGOS_TESTNET_BOOTSTRAP_PEERS: &[&str] = &[
    "/ip4/65.109.51.37/udp/3000/quic-v1/p2p/12D3KooWFrouXfmrR4nsLMtE7wu15DoMJ6VtoUtHinREZCvbWHar",
    "/ip4/65.109.51.37/udp/3001/quic-v1/p2p/12D3KooWJRGau8M1rjT7R5e4YYsgdFhsMX35nRDtMwCDjxQkXAHz",
    "/ip4/65.109.51.37/udp/3002/quic-v1/p2p/12D3KooWQXJavMDTRscjauFSgVAB1VLB6Rzpy2uY5SU9Tk7927tb",
    "/ip4/65.109.51.37/udp/50001/quic-v1/p2p/12D3KooWSQc7CcGtvWDPF1yCbBthFnQjprfCVHmfmNDUrSmqQsU1",
];

#[cfg(test)]
mod tests {
    use super::LOGOS_TESTNET_BOOTSTRAP_PEERS;

    #[test]
    fn testnet_bootstrap_peers_are_bedrock_network_peers() {
        assert_eq!(LOGOS_TESTNET_BOOTSTRAP_PEERS.len(), 4);
        for peer in LOGOS_TESTNET_BOOTSTRAP_PEERS {
            assert!(peer.contains("/quic-v1/p2p/"));
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
                .any(|peer| peer.contains("/udp/50001/"))
        );
    }
}
