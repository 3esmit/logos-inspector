const SUBSCRIPTION_EVENTS = [
    "storageStart",
    "storageStop",
    "storageConnect",
    "storageUploadProgress",
    "storageUploadDone",
    "storageDownloadProgress",
    "storageDownloadDone",
    "storageDownloadManifestDone",
    "storageRemoveDone"
]

function subscriptionEvents() {
    return SUBSCRIPTION_EVENTS.slice(0)
}
