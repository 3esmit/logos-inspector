pub(crate) const LOGOS_TESTNET_PRESET: &str = "logos.test";
pub(crate) const LOGOS_TESTNET_CHANNEL_ID: &str =
    "0101010101010101010101010101010101010101010101010101010101010101";
pub(crate) const LOCAL_BEDROCK_ENDPOINT: &str = "http://127.0.0.1:8080/";
pub(crate) const LOCAL_INDEXER_ENDPOINT: &str = "http://127.0.0.1:8779/";
pub(crate) const LEZ_TESTNET_SEQUENCER_ENDPOINT: &str = "https://testnet.lez.logos.co/";

/// Bedrock network peers published with blockchain 0.2.4. The configuration
/// generator derives IBD peers from the terminal `/p2p/<peer-id>` component;
/// omitting it leaves the IBD peer list empty even when IBD is enabled.
/// These are not Blend service locators (ports 3400-3402/50002).
/// Keep the addresses and IDs aligned with the compatible module deployment.
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
    fn testnet_bootstrap_peers_preserve_release_network_addresses_and_ids() {
        // Network peers from the blockchain 0.2.4 release instructions.
        let expected = [
            (3000, "12D3KooWFrouXfmrR4nsLMtE7wu15DoMJ6VtoUtHinREZCvbWHar"),
            (3001, "12D3KooWJRGau8M1rjT7R5e4YYsgdFhsMX35nRDtMwCDjxQkXAHz"),
            (3002, "12D3KooWQXJavMDTRscjauFSgVAB1VLB6Rzpy2uY5SU9Tk7927tb"),
            (
                50001,
                "12D3KooWSQc7CcGtvWDPF1yCbBthFnQjprfCVHmfmNDUrSmqQsU1",
            ),
        ];
        assert_eq!(LOGOS_TESTNET_BOOTSTRAP_PEERS.len(), expected.len());
        for (peer, (port, peer_id)) in LOGOS_TESTNET_BOOTSTRAP_PEERS.iter().zip(expected) {
            assert_eq!(
                *peer,
                format!("/ip4/65.109.51.37/udp/{port}/quic-v1/p2p/{peer_id}")
            );
            assert_eq!(peer.rsplit_once("/p2p/").map(|(_, id)| id), Some(peer_id));
            assert!(!peer.contains("/udp/340"));
            assert!(!peer.contains("/udp/50002"));
        }
    }
}
