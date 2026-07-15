#pragma once

#include <cstdint>

#include "logos_inspector_core.h"

class LogosInspectorHostTransport
{
public:
    using IngestModuleEventFn = int32_t (*)(
        LogosInspectorCore*,
        const char*,
        const char*,
        const char*);
    using SetRuntimeModuleEventHealthFn = int32_t (*)(
        LogosInspectorCore*,
        int32_t);

    virtual ~LogosInspectorHostTransport() = default;

    LogosInspectorHostTransport(const LogosInspectorHostTransport&) = delete;
    LogosInspectorHostTransport& operator=(const LogosInspectorHostTransport&) = delete;

    virtual bool bindCore(
        LogosInspectorCore* core,
        IngestModuleEventFn ingest,
        SetRuntimeModuleEventHealthFn setEventHealth) noexcept = 0;
    virtual bool activate() noexcept = 0;
    virtual LogosInspectorHostTransportV1 vtable() noexcept = 0;
    virtual bool ownsRuntimeModuleEvents() const noexcept = 0;
    virtual void close() noexcept = 0;

protected:
    LogosInspectorHostTransport() = default;
};
