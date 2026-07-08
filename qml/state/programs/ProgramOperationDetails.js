.import "../../utils/UiFormat.js" as UiFormat

function deployProgramOperationDetail(value) {
    const report = value || {}
    const program = String(report.program_id_base58 || report.program_id_hex || "")
    const tx = String(report.deployment_tx_hash || "")
    if (program.length > 0 && tx.length > 0) {
        return qsTr("%1, tx %2").arg(UiFormat.shortHash(program)).arg(UiFormat.shortHash(tx))
    }
    if (tx.length > 0) {
        return qsTr("tx %1").arg(UiFormat.shortHash(tx))
    }
    return qsTr("submitted")
}

function idlInstructionOperationDetail(value) {
    const report = value || {}
    const tx = String(report.tx_hash || report.txHash || "")
    if (tx.length > 0) {
        return qsTr("%1 %2, tx %3")
            .arg(String(report.mode || "tx"))
            .arg(String(report.instruction || "instruction"))
            .arg(UiFormat.shortHash(tx))
    }
    const words = Array.isArray(report.instruction_words) ? report.instruction_words.length : 0
    return qsTr("%1 %2, %3 word(s)")
        .arg(String(report.mode || "preview"))
        .arg(String(report.instruction || "instruction"))
        .arg(words)
}
