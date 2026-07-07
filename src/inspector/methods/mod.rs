mod catalog;
mod commands;

pub(crate) use catalog::{
    OperationDomain, OperationMethod, normalized_operation_method, operation_cancellable,
    operation_uses_mutating_flag,
};
pub(crate) use commands::{OperationRunner, handle_operation_command};
