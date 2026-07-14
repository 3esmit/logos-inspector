#pragma once

#include <functional>
#include <memory>
#include <string>

#include "logos_inspector_core.h"
#include "logos_module_context.h"

class LogosInspectorAsyncBridge;
class LogosInspectorHostTransport;

class LogosInspectorImpl : public LogosModuleContext
{
public:
    using HostTransportFactory =
        std::function<std::unique_ptr<LogosInspectorHostTransport>()>;

    LogosInspectorImpl();
    LogosInspectorImpl(HostTransportFactory hostTransportFactory);
    ~LogosInspectorImpl();

    LogosInspectorImpl(const LogosInspectorImpl&) = delete;
    LogosInspectorImpl& operator=(const LogosInspectorImpl&) = delete;

    /// Calls a Logos Inspector method with a JSON array argument string.
    std::string call(const std::string& method, const std::string& argsJson);

    /// Calls any module through the shared inspector bridge.
    std::string callModule(const std::string& module, const std::string& method, const std::string& argsJson);

    /// Starts an asynchronous Logos Inspector method call.
    std::string callAsync(
        const std::string& correlationId,
        const std::string& method,
        const std::string& argsJson);

    /// Starts an asynchronous call through the shared module bridge.
    std::string callModuleAsync(
        const std::string& correlationId,
        const std::string& module,
        const std::string& method,
        const std::string& argsJson);

    /// Polls an asynchronous call without consuming its terminal response.
    std::string pollAsync(const std::string& token);

    /// Requests cancellation without consuming or releasing the response.
    std::string cancelAsync(const std::string& token);

    /// Releases one asynchronous call and its retained response.
    std::string releaseAsync(const std::string& token);

    /// Reports the enabled asynchronous bridge wire schema or unavailable state.
    std::string asyncBridgeSchema();

    /// True only while native event subscription and retry ownership is healthy.
    bool logosInspectorOwnsRuntimeModuleEvents();

    /// Returns this module package version.
    std::string moduleVersion();

protected:
    void onContextReady() override;

private:
    HostTransportFactory hostTransportFactory_;
    std::unique_ptr<LogosInspectorAsyncBridge> asyncBridge_;
};
