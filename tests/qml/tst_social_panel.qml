import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/components"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "SocialPanel"
    when: windowShown
    width: 720
    height: 640

    BridgeHostFixture {
        id: fakeHost
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    Theme {
        id: theme
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        SocialPanel {
            id: panel

            theme: theme
            model: model
            topic: "/topic/comment"
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        model.shell.busy = false
        model.social.socialCommentState = ({})
        model.social.socialCommentRevision += 1
        findChild(panel, "commentBody").text = "Retry this comment"
    }

    function test_terminal_send_error_is_visible_and_retains_draft() {
        model.social.socialCommentState = ({
                "/topic/comment": {
                    rows: [],
                    cursor: "",
                    loading: false,
                    error: "",
                    exhausted: false,
                    sending: false,
                    sendError: "Delivery rejected the comment."
                }
            })
        model.social.socialCommentRevision += 1

        const warning = findChild(panel, "commentSendError")
        const hint = findChild(panel, "commentSendHint")
        const body = findChild(panel, "commentBody")
        tryCompare(warning, "visible", true)
        compare(warning.message, "Delivery rejected the comment.")
        compare(hint.text, "Delivery rejected the comment.")
        compare(body.text, "Retry this comment")
    }
}
