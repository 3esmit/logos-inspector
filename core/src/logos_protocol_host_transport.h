#pragma once

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <memory>

#include "logos_inspector_host_transport.h"
#include "logos_protocol.h"

struct LogosProtocolApi
{
    using ClientCreateFn = lp_client* (*)(
        const char*,
        const char*,
        const char*,
        const char*);
    using ClientDestroyFn = void (*)(lp_client*);
    using InvokeAsyncFn = int (*)(
        lp_client*,
        const char*,
        const char*,
        int,
        lp_result_cb,
        void*);
    using SubscribeFn = lp_subscription* (*)(
        lp_client*,
        const char*,
        lp_event_cb,
        void*);
    using UnsubscribeFn = void (*)(lp_subscription*);

    ClientCreateFn clientCreate = nullptr;
    ClientDestroyFn clientDestroy = nullptr;
    InvokeAsyncFn invokeAsync = nullptr;
    SubscribeFn subscribe = nullptr;
    UnsubscribeFn unsubscribe = nullptr;

    static LogosProtocolApi production() noexcept;
};

struct LogosProtocolHostTransportLimits
{
    std::size_t maxPendingRequests = 128;
    std::size_t maxSingleRequestBytes = std::size_t { 1024 } * 1024;
    std::size_t maxRetainedRequestBytes = std::size_t { 8 } * 1024 * 1024;
    std::size_t maxSingleResultBytes = std::size_t { 16 } * 1024 * 1024;
    std::size_t maxQueuedEvents = 256;
    std::size_t maxSingleEventBytes = std::size_t { 1024 } * 1024;
    std::size_t maxQueuedEventBytes = std::size_t { 8 } * 1024 * 1024;
    int invokeTimeoutMs = 20'000;
    std::chrono::milliseconds retryDelay = std::chrono::milliseconds(1);
};

class LogosProtocolHostTransport final : public LogosInspectorHostTransport
{
public:
    LogosProtocolHostTransport();
    LogosProtocolHostTransport(
        LogosProtocolApi protocolApi,
        LogosProtocolHostTransportLimits limits);
    ~LogosProtocolHostTransport() override;

    LogosProtocolHostTransport(const LogosProtocolHostTransport&) = delete;
    LogosProtocolHostTransport& operator=(const LogosProtocolHostTransport&) = delete;
    LogosProtocolHostTransport(LogosProtocolHostTransport&&) = delete;
    LogosProtocolHostTransport& operator=(LogosProtocolHostTransport&&) = delete;

    /// Binds event ingress before activation. The core remains caller-owned.
    bool bindCore(
        LogosInspectorCore* core,
        IngestModuleEventFn ingest,
        SetRuntimeModuleEventHealthFn setEventHealth) noexcept override;

    /// Creates all allowlisted clients and atomically arms the event catalog.
    bool activate() noexcept override;

    /// Stable C transport interface. Its context remains valid until destruction.
    LogosInspectorHostTransportV1 vtable() noexcept override;

    /// True only while every native subscription and retry policy is healthy.
    bool ownsRuntimeModuleEvents() const noexcept override;

    /// Idempotent shutdown. Safe to call concurrently from multiple threads.
    void close() noexcept override;

private:
    class Impl;
    std::unique_ptr<Impl> impl_;
};
