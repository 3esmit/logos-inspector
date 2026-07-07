use std::sync::atomic::AtomicBool;

use anyhow::{Result, bail};
use reqwest::Method;
use serde_json::Value;

use super::{
    NodeOperationRegistry, NodeOperationRequest, chain, delivery, local_nodes, storage, wallet,
};

pub(super) async fn execute_node_operation(
    request: NodeOperationRequest,
    registry: &NodeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    match request.method.as_str() {
        "storageManifests" => storage::execute_storage_manifests(&request).await,
        "storageDownloadManifest" => storage::execute_storage_download_manifest(&request).await,
        "storageFetch" => storage::execute_storage_fetch(&request).await,
        "storageUploadUrl" => storage::execute_storage_upload(&request).await,
        "storageDownloadToUrl" => {
            storage::execute_storage_download(&request, registry, operation_id, cancel_requested)
                .await
        }
        "storageRemove" => storage::execute_storage_remove(&request).await,
        "deliverySubscribe" => {
            delivery::execute_delivery_subscription(&request, Method::POST, "subscribe").await
        }
        "deliveryUnsubscribe" => {
            delivery::execute_delivery_subscription(&request, Method::DELETE, "unsubscribe").await
        }
        "deliverySend" => delivery::execute_delivery_send(&request).await,
        "deliveryCreateNode" => {
            delivery::execute_delivery_module_action(&request, "createNode").await
        }
        "deliveryStart" => delivery::execute_delivery_module_action(&request, "start").await,
        "deliveryStop" => delivery::execute_delivery_module_action(&request, "stop").await,
        "deliveryStoreQuery" => delivery::execute_delivery_store_query(&request).await,
        "localNodesAction" => local_nodes::execute_local_nodes_action(&request).await,
        "localWalletCreateAccount" => wallet::execute_wallet_create_account(&request).await,
        "localWalletSendTransaction" => wallet::execute_wallet_send_transaction(&request).await,
        "localWalletInstructionSubmit" => wallet::execute_wallet_instruction_submit(&request).await,
        "localWalletCommand" => wallet::execute_wallet_command(&request).await,
        "localWalletDeployProgram" => wallet::execute_wallet_deploy_program(&request).await,
        "localWalletSyncPrivate" => wallet::execute_wallet_sync_private(&request).await,
        "localWalletAccounts" => wallet::execute_wallet_accounts(&request).await,
        "blockchainNode" => chain::execute_blockchain_node(&request).await,
        "blockchainBlocks" => chain::execute_blockchain_blocks(&request).await,
        "blockchainLiveBlocks" => chain::execute_blockchain_live_blocks(&request).await,
        "blockchainBlock" => chain::execute_blockchain_block(&request).await,
        "blockchainTransaction" => chain::execute_blockchain_transaction(&request).await,
        "head" => chain::execute_execution_head(&request).await,
        "programs" => chain::execute_programs(&request).await,
        "block" => chain::execute_sequencer_block(&request).await,
        "sequencerBlocks" => chain::execute_sequencer_blocks(&request).await,
        "transaction" => chain::execute_sequencer_transaction(&request).await,
        "inspectTransaction" => chain::execute_inspect_transaction(&request).await,
        "traceTransaction" => chain::execute_trace_transaction(&request).await,
        "account" => chain::execute_account_operation(&request).await,
        "indexerHealth" => chain::execute_indexer_health_operation(&request).await,
        "indexerStatus" => chain::execute_indexer_status_operation(&request).await,
        "indexerFinalizedHead" => chain::execute_indexer_finalized_head(&request).await,
        "indexerBlocks" => chain::execute_indexer_blocks_operation(&request).await,
        "indexerBlockByHash" => chain::execute_indexer_block_by_hash_operation(&request).await,
        "indexerTransferRecipients" => {
            chain::execute_indexer_transfer_recipients_operation(&request).await
        }
        _ => bail!("unknown node operation method `{}`", request.method),
    }
}
