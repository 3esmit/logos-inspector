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
            topic: "/cryptarchia/account/comment/comments"
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        model.shell.busy = false
        model.shell.currentView = "overview"
        model.navigationBackStack = []
        model.navigationForwardStack = []
        model.networkConnectorConfig = ({
            scopes: {
                delivery: {
                    connector_id: "direct_delivery_rest",
                    provenance: "test"
                }
            }
        })
        model.messagingSourceMode = "rest"
        model.messagingStorePeerAddress = ""
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                    key: "delivery",
                    label: "Delivery",
                    status: "available",
                    sub_capabilities: ["delivery.store.query", "delivery.send"]
                }]
        })
        model.social.socialCommentState = ({})
        model.social.socialCommentRevision += 1
        panel.topic = "/cryptarchia/account/comment/comments"
        findChild(panel, "commentBody").text = "Retry this comment"
        wait(0)
    }

    function test_terminal_send_error_is_visible_and_retains_draft() {
        model.social.socialCommentState = ({
                "/cryptarchia/account/comment/comments": {
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

    function test_identity_selector_exposes_semantic_accessibility() {
        const identity = findChild(panel, "commentIdentity")

        verify(identity !== null)
        compare(identity.Accessible.name, "Comment identity")
        compare(identity.Accessible.description, identity.displayText)
    }

    function test_comment_card_exposes_author_body_and_time() {
        model.social.socialCommentState = ({
                "/cryptarchia/account/comment/comments": {
                    rows: [{
                            key: "comment-1",
                            displayName: "Pseudonym 7",
                            body: "Accessible transaction comment",
                            createdAt: "2026-07-16T20:31:00Z"
                        }],
                    cursor: "",
                    loading: false,
                    error: "",
                    exhausted: true,
                    sending: false,
                    sendError: ""
                }
            })
        model.social.socialCommentRevision += 1

        const card = findChild(panel, "socialCommentCard")
        verify(card !== null)
        tryCompare(card, "visible", true)
        compare(card.Accessible.role, Accessible.StaticText)
        compare(card.Accessible.name,
                "Pseudonym 7. Accessible transaction comment")
        compare(card.Accessible.description,
                panel.shortTime("2026-07-16T20:31:00Z"))
    }

    function test_store_provider_configuration_action_opens_delivery_settings() {
        model.messagingSourceMode = "logoscore_cli"
        model.networkConnectorConfig = ({
            scopes: {
                delivery: {
                    connector_id: "logoscore_cli_delivery_module",
                    provenance: "test"
                }
            }
        })
        wait(0)

        const configure = findChild(panel, "configureStoreProviderButton")
        verify(configure !== null)
        tryCompare(configure, "visible", true)
        compare(configure.Accessible.name, "Configure Store provider")
        verify(configure.width >= configure.implicitWidth)

        mouseClick(configure, configure.width / 2, configure.height / 2)

        tryCompare(model.shell, "currentView", "settings")
        compare(model.shell.settingsSection, "network")
        compare(model.shell.settingsNetworkSection, "messaging")

        model.messagingStorePeerAddress = "/dns4/provider.example/tcp/30303/p2p/peer"
        tryCompare(configure, "visible", false)
    }

    function test_store_provider_configuration_action_stays_hidden_for_other_failures() {
        model.messagingSourceMode = "logoscore_cli"
        model.networkConnectorConfig = ({
            scopes: {
                delivery: {
                    connector_id: "logoscore_cli_delivery_module",
                    provenance: "test"
                }
            }
        })
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                    key: "delivery",
                    label: "Delivery",
                    status: "unavailable",
                    sub_capabilities: ["delivery.store.query"],
                    unavailable_sub_capabilities: ["delivery.store.query"]
                }]
        })
        wait(0)

        const configure = findChild(panel, "configureStoreProviderButton")
        verify(configure !== null)
        tryCompare(configure, "visible", false)
    }

    function test_store_provider_configuration_action_stays_hidden_for_invalid_topic() {
        model.messagingSourceMode = "logoscore_cli"
        model.networkConnectorConfig = ({
            scopes: {
                delivery: {
                    connector_id: "logoscore_cli_delivery_module",
                    provenance: "test"
                }
            }
        })
        panel.topic = "/cryptarchia/account/comment/not-comments"
        wait(0)

        const configure = findChild(panel, "configureStoreProviderButton")
        verify(configure !== null)
        tryCompare(configure, "visible", false)
    }
}
