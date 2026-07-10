import QtQuick
import QtTest
import "../../qml/state/programs/ProgramContextPresentation.js" as ProgramContextPresentation
import "../../qml/state/programs/ProgramResultPresentation.js" as ProgramResultPresentation

TestCase {
    name: "ProgramContextPresentation"

    QtObject {
        id: page

        property var model: programModel
        property bool hasResponse: true
        property var responseValue: null
        property var theme: ({ textMuted: "muted", warning: "warning", success: "success" })

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

    QtObject {
        id: programModel

        property string programTab: "programIds"
        property bool resultIsError: false
        property string resultTitle: "Program IDs"
        property string sharedIdlPolicy: "preferLocal"

        function canonicalProgramIdHex(value) {
            return String(value || "").length ? "0xabc" : ""
        }

        function idlEntriesForProgram(programId) {
            return String(programId || "") === "0xabc" ? [{ name: "Demo" }] : []
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

    function test_not_found_context_uses_program_context_summary() {
        const context = programContext()
        context.in_chain = false
        context.verification = "not_found"
        context.known_label = ""
        page.responseValue = context

        const rows = ProgramContextPresentation.rows(page, context)

        compare(ProgramContextPresentation.verificationText(context), "not in getProgramIds")
        compare(ProgramContextPresentation.responseProgramText(page, context), "Base...amId")
        compare(ProgramContextPresentation.responseProgramDelta(page, context), "not verified")
        compare(ProgramResultPresentation.responseProgramText(page), "Base...amId")
        compare(ProgramResultPresentation.responseProgramDelta(page), "not verified")
        compare(rows.length, 5)
        compare(rows[0].value, "not in getProgramIds")
        compare(rows[1].linkKind, "")
        compare(rows[2].linkKind, "")
    }

    function test_program_result_presentation_wraps_tab_and_result_state() {
        programModel.programTab = "binaries"
        page.hasResponse = false

        compare(ProgramResultPresentation.activeTabLabel(page), "Binaries")
        compare(ProgramResultPresentation.activeTabDelta(page), "File inspection")
        compare(ProgramResultPresentation.lastResultText(page), "Idle")
        compare(ProgramResultPresentation.lastResultColor(page), "muted")
        verify(ProgramResultPresentation.validProgramId(page, "program"))
    }

    function test_program_result_presentation_projects_rows() {
        page.responseValue = [
            { label: "Known", hex: "0xabc", base58: "Base58" }
        ]
        compare(ProgramResultPresentation.programTableRows(page)[0].knownIdl, "Demo")

        page.responseValue = {
            program_id_hex: "0xabc",
            program_id_base58: "Base58",
            deployment_tx_hash: "0x1111",
            path: "/tmp/program.bin",
            bytecode_len: 9
        }
        const rows = ProgramResultPresentation.programFileRows(page)
        compare(rows.length, 5)
        compare(rows[2].linkKind, "program")
        compare(ProgramResultPresentation.responseProgramText(page), "0xabc")
        compare(ProgramResultPresentation.responseProgramDelta(page), "9 bytes")
    }

    function test_program_result_presentation_projects_event_decode_rows() {
        page.responseValue = {
            event: "Transfer",
            consumed_bytes: 16,
            total_bytes: 16,
            decoded: ({ amount: 42, recipient: "Base58Recipient" }),
            rows: [
                { path: "amount", value: "42" },
                { path: "recipient", value: "Base58Recipient" }
            ]
        }

        verify(ProgramResultPresentation.isEventDecodeReport(page.responseValue))
        const rows = ProgramResultPresentation.eventDecodeRows(page)
        compare(rows.length, 2)
        compare(rows[0].label, "amount")
        compare(rows[0].value, "42")
        compare(rows[0].monospace, true)
        compare(rows[1].label, "recipient")
        compare(rows[1].value, "Base58Recipient")
        verify(!ProgramResultPresentation.isEventDecodeReport({ event: "Transfer", rows: [] }))
    }
}
