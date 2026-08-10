pub(crate) const LOGOS_TESTNET_PRESET: &str = "logos.test";
pub(crate) const LOGOS_TESTNET_CHANNEL_ID: &str =
    "0101010101010101010101010101010101010101010101010101010101010101";
pub(crate) const LOCAL_BEDROCK_ENDPOINT: &str = "http://127.0.0.1:8080/";
pub(crate) const LOCAL_INDEXER_ENDPOINT: &str = "http://127.0.0.1:8779/";
pub(crate) const LEZ_TESTNET_SEQUENCER_ENDPOINT: &str = "https://testnet.lez.logos.co/";

pub(crate) const LOGOS_TESTNET_BOOTSTRAP_PEERS: &[&str] = &[
    "/ip4/65.109.51.37/udp/3400/quic-v1",
    "/ip4/65.109.51.37/udp/3401/quic-v1",
    "/ip4/65.109.51.37/udp/3402/quic-v1",
    "/ip4/65.109.51.37/udp/50002/quic-v1",
];

#[cfg(test)]
mod tests {
    use super::LOGOS_TESTNET_BOOTSTRAP_PEERS;

    #[test]
    fn testnet_bootstrap_peers_match_current_deployment() {
        assert_eq!(
            LOGOS_TESTNET_BOOTSTRAP_PEERS,
            &[
                "/ip4/65.109.51.37/udp/3400/quic-v1",
                "/ip4/65.109.51.37/udp/3401/quic-v1",
                "/ip4/65.109.51.37/udp/3402/quic-v1",
                "/ip4/65.109.51.37/udp/50002/quic-v1",
            ]
        );
    }
}
