import QtQml
import "appmodel/AppModelModuleEvents.js" as AppModelModuleEvents

QtObject {
    id: root

    required property var bridge
    required property var model

    function install() {
        if (!bridge || !model) {
            return 0
        }
        const rows = AppModelModuleEvents.moduleEventSubscriptions(model)
        let count = 0
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            count += bridge.subscribeModuleEvents(String(row.moduleName || ""), row.events || [])
        }
        return count
    }

    function ingest(moduleName, eventName, args) {
        if (!model) {
            return false
        }
        return AppModelModuleEvents.handleModuleEvent(model, moduleName, eventName, args)
    }

    property Connections bridgeEvents: Connections {
        target: root.bridge
        ignoreUnknownSignals: true

        function onHostChanged() {
            Qt.callLater(function () {
                root.install()
            })
        }

        function onModuleEventReceived(moduleName, eventName, args) {
            root.ingest(moduleName, eventName, args)
        }
    }
}
