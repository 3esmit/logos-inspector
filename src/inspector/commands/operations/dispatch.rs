use std::sync::atomic::AtomicBool;

use anyhow::Result;
use reqwest::Method;
use serde_json::Value;

use super::{
    NodeOperationRegistry, NodeOperationRequest, OperationExecutor, chain, delivery, local_nodes,
    storage, wallet,
};

pub(super) async fn execute_node_operation(
    request: NodeOperationRequest,
    registry: &NodeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    match request.executor() {
        OperationExecutor::StorageManifests => storage::execute_storage_manifests(&request).await,
        OperationExecutor::StorageDownloadManifest => {
            storage::execute_storage_download_manifest(&request).await
        }
        OperationExecutor::StorageFetch => storage::execute_storage_fetch(&request).await,
        OperationExecutor::StorageUploadUrl => storage::execute_storage_upload(&request).await,
        OperationExecutor::StorageDownloadToUrl => {
            storage::execute_storage_download(&request, registry, operation_id, cancel_requested)
                .await
        }
        OperationExecutor::StorageRemove => storage::execute_storage_remove(&request).await,
        OperationExecutor::DeliverySubscribe => {
            delivery::execute_delivery_subscription(&request, Method::POST, "subscribe").await
        }
        OperationExecutor::DeliveryUnsubscribe => {
            delivery::execute_delivery_subscription(&request, Method::DELETE, "unsubscribe").await
        }
        OperationExecutor::DeliverySend => delivery::execute_delivery_send(&request).await,
        OperationExecutor::DeliveryCreateNode => {
            delivery::execute_delivery_module_action(&request, "createNode").await
        }
        OperationExecutor::DeliveryStart => {
            delivery::execute_delivery_module_action(&request, "start").await
        }
        OperationExecutor::DeliveryStop => {
            delivery::execute_delivery_module_action(&request, "stop").await
        }
        OperationExecutor::DeliveryStoreQuery => {
            delivery::execute_delivery_store_query(&request).await
        }
        OperationExecutor::LocalNodesAction => {
            local_nodes::execute_local_nodes_action(&request).await
        }
        OperationExecutor::LocalWalletCreateAccount => {
            wallet::execute_wallet_create_account(&request).await
        }
        OperationExecutor::LocalWalletSendTransaction => {
            wallet::execute_wallet_send_transaction(&request).await
        }
        OperationExecutor::LocalWalletInstructionSubmit => {
            wallet::execute_wallet_instruction_submit(&request).await
        }
        OperationExecutor::LocalWalletCommand => wallet::execute_wallet_command(&request).await,
        OperationExecutor::LocalWalletDeployProgram => {
            wallet::execute_wallet_deploy_program(&request).await
        }
        OperationExecutor::LocalWalletSyncPrivate => {
            wallet::execute_wallet_sync_private(&request).await
        }
        OperationExecutor::LocalWalletAccounts => wallet::execute_wallet_accounts(&request).await,
        OperationExecutor::BlockchainNode => chain::execute_blockchain_node(&request).await,
        OperationExecutor::BlockchainBlocks => chain::execute_blockchain_blocks(&request).await,
        OperationExecutor::BlockchainLiveBlocks => {
            chain::execute_blockchain_live_blocks(&request).await
        }
        OperationExecutor::BlockchainBlock => chain::execute_blockchain_block(&request).await,
        OperationExecutor::BlockchainTransaction => {
            chain::execute_blockchain_transaction(&request).await
        }
        OperationExecutor::Health => chain::execute_execution_health(&request).await,
        OperationExecutor::Head => chain::execute_execution_head(&request).await,
        OperationExecutor::Programs => chain::execute_programs(&request).await,
        OperationExecutor::Block => chain::execute_sequencer_block(&request).await,
        OperationExecutor::SequencerBlocks => chain::execute_sequencer_blocks(&request).await,
        OperationExecutor::Transaction => chain::execute_sequencer_transaction(&request).await,
        OperationExecutor::InspectTransaction => chain::execute_inspect_transaction(&request).await,
        OperationExecutor::TraceTransaction => chain::execute_trace_transaction(&request).await,
        OperationExecutor::Account => chain::execute_account_operation(&request).await,
        OperationExecutor::ResolveLezTarget => chain::execute_resolve_lez_target(&request).await,
        OperationExecutor::IndexerHealth => chain::execute_indexer_health_operation(&request).await,
        OperationExecutor::IndexerStatus => chain::execute_indexer_status_operation(&request).await,
        OperationExecutor::IndexerFinalizedHead => {
            chain::execute_indexer_finalized_head(&request).await
        }
        OperationExecutor::IndexerBlocks => chain::execute_indexer_blocks_operation(&request).await,
        OperationExecutor::IndexerBlockByHash => {
            chain::execute_indexer_block_by_hash_operation(&request).await
        }
        OperationExecutor::IndexerTransferRecipients => {
            chain::execute_indexer_transfer_recipients_operation(&request).await
        }
    }
}
