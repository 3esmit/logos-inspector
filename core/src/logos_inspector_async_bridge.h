#pragma once

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <memory>
#include <string>

#include "logos_inspector_core.h"

class LogosInspectorHostTransport;

struct LogosInspectorAsyncBridgeLimits
{
    std::size_t maxSlots = 128;
    std::size_t maxSingleInputBytes = std::size_t { 1024 } * 1024;
    std::size_t maxRetainedInputBytes = std::size_t { 8 } * 1024 * 1024;
    std::size_t maxRetainedResponseBytes = std::size_t { 16 } * 1024 * 1024;
    std::chrono::milliseconds entryTtl = std::chrono::minutes(5);
};

struct LogosInspectorCoreApi
{
    using NewWithHostTransportFn = LogosInspectorCore* (*)(const LogosInspectorHostTransportV1*);
    using CloseFn = void (*)(LogosInspectorCore*);
    using FreeFn = void (*)(LogosInspectorCore*);
    using CallFn = char* (*)(LogosInspectorCore*, const char*, const char*);
    using StringFreeFn = void (*)(char*);
    using CallModuleAsyncFn = int32_t (*)(
        LogosInspectorCore*,
        uint64_t,
        const char*,
        const char*,
        const char*,
        LogosInspectorCoreReplyFn,
        void*);
    using CancelFn = int32_t (*)(LogosInspectorCore*, uint64_t);
    using IngestModuleEventFn = int32_t (*)(
        LogosInspectorCore*,
        const char*,
        const char*,
        const char*);
    using SetRuntimeModuleEventHealthFn = int32_t (*)(
        LogosInspectorCore*,
        int32_t);

    NewWithHostTransportFn newWithHostTransport = nullptr;
    CloseFn close = nullptr;
    FreeFn free = nullptr;
    CallFn call = nullptr;
    StringFreeFn stringFree = nullptr;
    CallModuleAsyncFn callModuleAsync = nullptr;
    CancelFn cancel = nullptr;
    IngestModuleEventFn ingestModuleEvent = nullptr;
    SetRuntimeModuleEventHealthFn setRuntimeModuleEventHealth = nullptr;

    static LogosInspectorCoreApi production();
};

class LogosInspectorAsyncBridge
{
public:
    using Clock = std::function<std::chrono::steady_clock::time_point()>;

    explicit LogosInspectorAsyncBridge(
        std::unique_ptr<LogosInspectorHostTransport> hostTransport);
    LogosInspectorAsyncBridge(
        LogosInspectorCoreApi coreApi,
        LogosInspectorAsyncBridgeLimits limits,
        Clock clock,
        uint64_t tokenNamespace,
        std::unique_ptr<LogosInspectorHostTransport> hostTransport);
    ~LogosInspectorAsyncBridge();

    LogosInspectorAsyncBridge(const LogosInspectorAsyncBridge&) = delete;
    LogosInspectorAsyncBridge& operator=(const LogosInspectorAsyncBridge&) = delete;

    std::string call(const std::string& method, const std::string& argsJson);
    std::string callAsync(
        const std::string& correlationId,
        const std::string& method,
        const std::string& argsJson);
    std::string callModuleAsync(
        const std::string& correlationId,
        const std::string& module,
        const std::string& method,
        const std::string& argsJson);
    std::string pollAsync(const std::string& token);
    std::string cancelAsync(const std::string& token);
    std::string releaseAsync(const std::string& token);
    bool ownsRuntimeModuleEvents() const noexcept;

    /// Idempotent shutdown seam used by the owner and lifecycle tests.
    void close();

private:
    class Impl;
    std::unique_ptr<Impl> impl_;
};
