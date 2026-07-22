import QtQuick
import QtTest
import "../../qml/state/programs/ProgramResultPresentation.js" as ProgramResultPresentation

TestCase {
    id: testRoot

    name: "ProgramResultPresentation"

    QtObject {
        id: root

        property var responseValue: null

        function numberText(value) { return String(value) }
        function valueText(value) { return String(value) }
    }

    function test_program_file_summary_uses_presenter_hash_helper() {
        root.responseValue = {
            path: "/tmp/program.bin",
            bytecode_len: 282360,
            program_id_hex: "f614e573eb6feeebc7d20ab75293f017cdae83694871bd65ff0314e0824fba01",
            program_id_base58: "HZbmvE7dNLyvNvtjtkfXRaAeN5UqXMAxdFHz5oEphqGC",
            deployment_tx_hash: "7c7bd41c1a34ea094969319fbc16a01f82e1fd47201d4f4700c0422ddbba393c"
        }

        compare(ProgramResultPresentation.responseProgramText(root), "f614e573...4fba01")
        compare(ProgramResultPresentation.responseProgramDelta(root), "282360 bytes")
        compare(ProgramResultPresentation.programFileRows(root).length, 5)
    }
}
