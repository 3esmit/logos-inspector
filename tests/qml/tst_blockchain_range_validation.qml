import QtTest
import "../../qml/state/chain/BlockchainRangeValidation.js" as BlockchainRangeValidation

TestCase {
    name: "BlockchainRangeValidation"

    function test_empty_range_stays_quiet() {
        const result = BlockchainRangeValidation.validate("", "")
        verify(!result.valid)
        compare(result.message, "")
    }

    function test_incomplete_range_is_actionable() {
        const result = BlockchainRangeValidation.validate("10", "")
        verify(!result.valid)
        compare(result.message, "Enter both Slot from and Slot to.")
        compare(result.invalidField, "to")
    }

    function test_decimal_range_normalizes_for_transport() {
        const result = BlockchainRangeValidation.validate("10", "11")
        verify(result.valid)
        compare(result.slotFrom, 10)
        compare(result.slotTo, 11)
    }

    function test_non_decimal_forms_are_rejected() {
        for (const value of [
            "1e3", "1.0", "0x10", "-1", "+1", "one", "01", " 1", "1 "
        ]) {
            const result = BlockchainRangeValidation.validate(value, "1000")
            verify(!result.valid)
            compare(result.message,
                    "Slots must use unsigned decimal integers without signs, spaces, or leading zeros.")
            compare(result.invalidField, "from")
        }
    }

    function test_reversed_range_is_rejected() {
        const result = BlockchainRangeValidation.validate("20", "10")
        verify(!result.valid)
        compare(result.message, "Slot from must be less than or equal to Slot to.")
        compare(result.invalidField, "to")
    }

    function test_existing_page_window_is_maximum_valid_range() {
        const maximum = BlockchainRangeValidation.validate("0", "2000")
        const oversized = BlockchainRangeValidation.validate("0", "2001")

        compare(BlockchainRangeValidation.maximumSlotCount(), 2001)
        verify(maximum.valid)
        verify(!oversized.valid)
        compare(oversized.message,
                "Slot range cannot contain more than 2,001 slots.")
        compare(oversized.invalidField, "to")
    }

    function test_values_beyond_safe_integer_are_rejected() {
        const result = BlockchainRangeValidation.validate(
            "9007199254740992", "9007199254740992")
        verify(!result.valid)
        compare(result.message, "Slots exceed the supported numeric range.")
    }

    function test_mixed_parse_errors_mark_only_the_first_reported_error_kind() {
        const malformedFrom = BlockchainRangeValidation.validate(
            "bad", "9007199254740992")
        verify(!malformedFrom.valid)
        compare(malformedFrom.message,
                "Slots must use unsigned decimal integers without signs, spaces, or leading zeros.")
        compare(malformedFrom.invalidField, "from")

        const malformedTo = BlockchainRangeValidation.validate(
            "9007199254740992", "bad")
        verify(!malformedTo.valid)
        compare(malformedTo.message,
                "Slots must use unsigned decimal integers without signs, spaces, or leading zeros.")
        compare(malformedTo.invalidField, "to")
    }
}
