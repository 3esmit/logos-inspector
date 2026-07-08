use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfirmationPolicy {
    LocalNodeAction,
    WalletCreateAccount,
    WalletSendTransaction,
    WalletInstructionSubmit,
    WalletCommand,
    WalletDeployProgram,
    WalletSyncPrivate,
}

impl ConfirmationPolicy {
    #[must_use]
    pub(crate) fn token(self) -> &'static str {
        match self {
            Self::LocalNodeAction => "confirm-local-node-action",
            Self::WalletCreateAccount => "confirm-create-account",
            Self::WalletSendTransaction => "confirm-send-transaction",
            Self::WalletInstructionSubmit => "confirm-idl-instruction",
            Self::WalletCommand => "confirm-wallet-command",
            Self::WalletDeployProgram => "confirm-deploy-program",
            Self::WalletSyncPrivate => "confirm-sync-private",
        }
    }

    #[must_use]
    pub(crate) fn missing_message(self) -> &'static str {
        match self {
            Self::LocalNodeAction => "local node action requires explicit confirmation",
            Self::WalletCreateAccount => "wallet account creation requires explicit confirmation",
            Self::WalletSendTransaction => "wallet transaction send requires explicit confirmation",
            Self::WalletInstructionSubmit => "IDL instruction send requires explicit confirmation",
            Self::WalletCommand => "wallet command requires explicit confirmation",
            Self::WalletDeployProgram => "program deployment requires explicit confirmation",
            Self::WalletSyncPrivate => "private wallet sync requires explicit confirmation",
        }
    }

    pub(crate) fn require(self, confirmation: Option<&str>) -> Result<()> {
        if confirmation == Some(self.token()) {
            return Ok(());
        }
        bail!("{}", self.missing_message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_policy_enforces_token() {
        let policy = ConfirmationPolicy::WalletDeployProgram;

        assert!(policy.require(Some(policy.token())).is_ok());
        assert_eq!(
            policy.require(None).err().map(|error| error.to_string()),
            Some("program deployment requires explicit confirmation".to_owned())
        );
    }
}
