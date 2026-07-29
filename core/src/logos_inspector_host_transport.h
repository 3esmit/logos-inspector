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
    using IngestModuleInstanceEventFn = int32_t (*)(
        LogosInspectorCore*,
        const char*,
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

    /// Optional V2 bind point. V1-only transports retain their existing
    /// behavior and cannot receive scoped module events.
    virtual bool bindCoreV2(
        LogosInspectorCore* core,
        IngestModuleEventFn ingest,
        IngestModuleInstanceEventFn ingestInstance,
        SetRuntimeModuleEventHealthFn setEventHealth) noexcept
    {
        static_cast<void>(ingestInstance);
        return bindCore(core, ingest, setEventHealth);
    }
    virtual bool activate() noexcept = 0;
    virtual LogosInspectorHostTransportV1 vtable() noexcept = 0;

    /// Optional additive V2 transport vtable. The default preserves a V1
    /// prefix, so the async bridge will continue with its V1 constructor.
    virtual LogosInspectorHostTransportV2 vtableV2() noexcept
    {
        LogosInspectorHostTransportV2 result {};
        result.v1 = vtable();
        return result;
    }
    virtual bool ownsRuntimeModuleEvents() const noexcept = 0;
    virtual void close() noexcept = 0;

protected:
    LogosInspectorHostTransport() = default;
};
