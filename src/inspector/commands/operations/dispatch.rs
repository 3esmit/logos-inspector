use std::sync::atomic::AtomicBool;

use anyhow::Result;
use reqwest::Method;
use serde_json::Value;

use super::{
    OperationMethod, RuntimeOperationRegistry, RuntimeOperationRequest, chain, delivery,
    local_nodes, storage, wallet,
};

pub(super) async fn execute_runtime_operation(
    request: RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    match request.method() {
        OperationMethod::StorageManifests => storage::execute_storage_manifests(&request).await,
        OperationMethod::StorageDownloadManifest => {
            storage::execute_storage_download_manifest(&request).await
        }
        OperationMethod::StorageFetch => storage::execute_storage_fetch(&request).await,
        OperationMethod::StorageUploadUrl => storage::execute_storage_upload(&request).await,
        OperationMethod::StorageDownloadToUrl => {
            storage::execute_storage_download(&request, registry, operation_id, cancel_requested)
                .await
        }
        OperationMethod::StorageRemove => storage::execute_storage_remove(&request).await,
        OperationMethod::DeliverySubscribe => {
            delivery::execute_delivery_subscription(&request, Method::POST, "subscribe").await
        }
        OperationMethod::DeliveryUnsubscribe => {
            delivery::execute_delivery_subscription(&request, Method::DELETE, "unsubscribe").await
        }
        OperationMethod::DeliverySend => delivery::execute_delivery_send(&request).await,
        OperationMethod::DeliveryCreateNode => {
            delivery::execute_delivery_module_action(&request, "createNode").await
        }
        OperationMethod::DeliveryStart => {
            delivery::execute_delivery_module_action(&request, "start").await
        }
        OperationMethod::DeliveryStop => {
            delivery::execute_delivery_module_action(&request, "stop").await
        }
        OperationMethod::DeliveryStoreQuery => {
            delivery::execute_delivery_store_query(&request).await
        }
        OperationMethod::LocalNodesAction => {
            local_nodes::execute_local_nodes_action(&request).await
        }
        OperationMethod::LocalWalletCreateAccount => {
            wallet::execute_wallet_create_account(&request).await
        }
        OperationMethod::LocalWalletSendTransaction => {
            wallet::execute_wallet_send_transaction(&request).await
        }
        OperationMethod::LocalWalletInstructionSubmit => {
            wallet::execute_wallet_instruction_submit(&request).await
        }
        OperationMethod::LocalWalletCommand => wallet::execute_wallet_command(&request).await,
        OperationMethod::LocalWalletDeployProgram => {
            wallet::execute_wallet_deploy_program(&request).await
        }
        OperationMethod::LocalWalletSyncPrivate => {
            wallet::execute_wallet_sync_private(&request).await
        }
        OperationMethod::LocalWalletAccounts => wallet::execute_wallet_accounts(&request).await,
        OperationMethod::BlockchainNode => chain::execute_blockchain_node(&request).await,
        OperationMethod::BlockchainBlocks => chain::execute_blockchain_blocks(&request).await,
        OperationMethod::BlockchainLiveBlocks => {
            chain::execute_blockchain_live_blocks(&request).await
        }
        OperationMethod::BlockchainBlock => chain::execute_blockchain_block(&request).await,
        OperationMethod::BlockchainTransaction => {
            chain::execute_blockchain_transaction(&request).await
        }
        OperationMethod::Health => chain::execute_execution_health(&request).await,
        OperationMethod::Head => chain::execute_execution_head(&request).await,
        OperationMethod::Programs => chain::execute_programs(&request).await,
        OperationMethod::Block => chain::execute_sequencer_block(&request).await,
        OperationMethod::SequencerBlocks => chain::execute_sequencer_blocks(&request).await,
        OperationMethod::Transaction => chain::execute_sequencer_transaction(&request).await,
        OperationMethod::InspectTransaction => chain::execute_inspect_transaction(&request).await,
        OperationMethod::TraceTransaction => chain::execute_trace_transaction(&request).await,
        OperationMethod::Account => chain::execute_account_operation(&request).await,
        OperationMethod::ResolveLezTarget => chain::execute_resolve_lez_target(&request).await,
        OperationMethod::IndexerHealth => chain::execute_indexer_health_operation(&request).await,
        OperationMethod::IndexerStatus => chain::execute_indexer_status_operation(&request).await,
        OperationMethod::IndexerFinalizedHead => {
            chain::execute_indexer_finalized_head(&request).await
        }
        OperationMethod::IndexerBlocks => chain::execute_indexer_blocks_operation(&request).await,
        OperationMethod::IndexerBlockByHash => {
            chain::execute_indexer_block_by_hash_operation(&request).await
        }
        OperationMethod::IndexerTransferRecipients => {
            chain::execute_indexer_transfer_recipients_operation(&request).await
        }
    }
}
