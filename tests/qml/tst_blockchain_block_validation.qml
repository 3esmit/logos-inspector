import QtTest
import "../../qml/state/chain/BlockchainBlockValidation.js" as BlockchainBlockValidation

TestCase {
    id: root

    name: "BlockchainBlockValidation"

    function test_empty_value_stays_quiet() {
        for (const value of ["", "   "]) {
            const result = BlockchainBlockValidation.validate(value)
            verify(!result.valid)
            compare(result.message, "")
            compare(result.blockId, "")
        }
    }

    function test_valid_values_are_canonicalized_for_transport() {
        const canonical = "ab".repeat(32)
        for (const value of [canonical, "  0x" + canonical.toUpperCase() + "  "]) {
            const result = BlockchainBlockValidation.validate(value)

            verify(result.valid)
            compare(result.message, "")
            compare(result.blockId, canonical)
        }
    }

    function test_non_block_ids_are_rejected() {
        const invalidValues = [
            "block/a",
            "block\\a",
            ".",
            "..",
            "%2e%2e",
            "a\nb",
            "a\rb",
            "a\tb",
            "a".repeat(63),
            "a".repeat(65),
            "g".repeat(64)
        ]
        for (const value of invalidValues) {
            const result = BlockchainBlockValidation.validate(value)
            verify(!result.valid)
            compare(result.message,
                    "Block ID must be 64 hexadecimal characters (optional 0x prefix).")
            compare(result.blockId, "")
        }
    }
}
