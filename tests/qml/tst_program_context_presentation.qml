import QtQuick
import QtTest
import "../../qml/state/programs/ProgramContextPresentation.js" as ProgramContextPresentation

TestCase {
    name: "ProgramContextPresentation"

    QtObject {
        id: page

        function idlFieldCount(text) {
            const value = JSON.parse(String(text || "{}"))
            return Array.isArray(value.instructions) ? value.instructions.length : 0
        }

        function isProgramFile(value) {
            return value && value.kind === "program_file"
        }

        function numberText(value) {
            return String(Number(value || 0))
        }

        function shortHash(value) {
            const text = String(value || "")
            return text.length > 8 ? text.slice(0, 4) + "..." + text.slice(text.length - 4) : text
        }

        function valueText(value) {
            return value === undefined || value === null || value === "" ? "-" : String(value)
        }
    }

    function programContext() {
        return {
            type: "program",
            program_id: "program-full-id",
            program_id_base58: "Base58ProgramId",
            program_id_hex: "0xabcdef",
            in_chain: true,
            known_label: "Known",
            idls: [
                { name: "IDL", programId: "Base58ProgramId", json: "{\"instructions\":[{},{}]}" }
            ],
            recent_transactions: [
                { hash: "0123456789abcdef", block_id: 7, kind: "deploy", ops: 3 }
            ],
            account: { address: "Base58ProgramId" }
        }
    }

    function test_summary_text_uses_program_context_adapter() {
        const context = programContext()

        compare(ProgramContextPresentation.responseProgramText(page, context), "Base...amId")
        compare(ProgramContextPresentation.responseProgramDelta(page, context), "verified in chain")
        compare(ProgramContextPresentation.responseProgramText(page, [1, 2, 3]), "3")
        compare(ProgramContextPresentation.responseProgramDelta(page, { kind: "program_file", program_id_hex: "abcdef", bytecode_len: 12 }), "12 bytes")
    }

    function test_rows_project_program_context() {
        const rows = ProgramContextPresentation.rows(page, programContext())

        compare(rows.length, 5)
        compare(rows[0].label, "Known program")
        compare(rows[0].value, "yes")
        compare(rows[1].linkKind, "program")
        compare(rows[3].linkKind, "account")
    }

    function test_detail_lists_and_account_are_capped_and_formatted() {
        const context = programContext()
        const idlRows = ProgramContextPresentation.idlRows(page, context)
        const transactionRows = ProgramContextPresentation.transactionRows(page, context)

        compare(idlRows.length, 1)
        compare(idlRows[0].detail, "2 field(s), program Base...amId")
        compare(transactionRows.length, 1)
        compare(transactionRows[0].title, "0123...cdef")
        compare(transactionRows[0].detail, "block 7, deploy, 3 word(s)")
        compare(ProgramContextPresentation.account(context).address, "Base58ProgramId")
    }

    function test_unverified_context_exposes_verification_detail() {
        const context = programContext()
        context.in_chain = false
        context.verification = "unavailable"
        context.verification_detail = "rpc unavailable"
        context.program_id_base58 = ""

        const rows = ProgramContextPresentation.rows(page, context)

        compare(ProgramContextPresentation.verificationText(context), "verification unavailable")
        compare(rows[1].linkKind, "")
        compare(rows[2].value, "0xabcdef")
        compare(rows[rows.length - 1].value, "rpc unavailable")
    }
}
