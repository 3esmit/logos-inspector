#include "logos_inspector_impl.h"

#include "logos_inspector_async_bridge.h"
#include "logos_inspector_host_transport.h"

#include <exception>
#include <utility>

namespace {
#ifndef LOGOS_INSPECTOR_MODULE_VERSION
#define LOGOS_INSPECTOR_MODULE_VERSION "unknown"
#endif
constexpr const char* kModuleVersion = LOGOS_INSPECTOR_MODULE_VERSION;

std::string jsonEscape(const std::string& value)
{
    static constexpr char kHex[] = "0123456789abcdef";
    std::string escaped;
    escaped.reserve(value.size());
    for (const unsigned char ch : value) {
        switch (ch) {
        case '\\':
            escaped += "\\\\";
            break;
        case '"':
            escaped += "\\\"";
            break;
        case '\b':
            escaped += "\\b";
            break;
        case '\f':
            escaped += "\\f";
            break;
        case '\n':
            escaped += "\\n";
            break;
        case '\r':
            escaped += "\\r";
            break;
        case '\t':
            escaped += "\\t";
            break;
        default:
            if (ch < 0x20) {
                escaped += "\\u00";
                escaped += kHex[(ch >> 4) & 0x0f];
                escaped += kHex[ch & 0x0f];
            } else {
                escaped += static_cast<char>(ch);
            }
            break;
        }
    }
    return escaped;
}

std::string jsonError(const std::string& error)
{
    return "{\"ok\":false,\"value\":null,\"text\":\"\",\"error\":\""
        + jsonEscape(error) + "\"}";
}

template<typename Function>
std::string invokeAsyncBridge(
    LogosInspectorAsyncBridge* bridge,
    const char* unavailableError,
    Function&& function)
{
    if (bridge == nullptr) {
        return jsonError(unavailableError);
    }
    try {
        return std::forward<Function>(function)(*bridge);
    } catch (const std::exception& error) {
        return jsonError(std::string("asynchronous bridge call failed: ") + error.what());
    } catch (...) {
        return jsonError("asynchronous bridge call failed");
    }
}
}

LogosInspectorImpl::LogosInspectorImpl(HostTransportFactory hostTransportFactory)
    : hostTransportFactory_(std::move(hostTransportFactory))
{
}

LogosInspectorImpl::~LogosInspectorImpl()
{
    asyncBridge_.reset();
}

void LogosInspectorImpl::onContextReady()
{
    if (asyncBridge_ != nullptr || !hostTransportFactory_) {
        return;
    }
    try {
        std::unique_ptr<LogosInspectorHostTransport> hostTransport =
            hostTransportFactory_();
        if (hostTransport != nullptr) {
            asyncBridge_ = std::make_unique<LogosInspectorAsyncBridge>(
                std::move(hostTransport));
        }
    } catch (...) {
        asyncBridge_.reset();
    }
}

std::string LogosInspectorImpl::call(const std::string& method, const std::string& argsJson)
{
    return invokeAsyncBridge(
        asyncBridge_.get(),
        "logos inspector bridge is not initialized",
        [&](LogosInspectorAsyncBridge& bridge) { return bridge.call(method, argsJson); });
}

std::string LogosInspectorImpl::callModule(
    const std::string& module,
    const std::string& method,
    const std::string& argsJson)
{
    static_cast<void>(module);
    static_cast<void>(method);
    static_cast<void>(argsJson);
    return jsonError("synchronous module calls are unsupported; use callModuleAsync");
}

std::string LogosInspectorImpl::callAsync(
    const std::string& correlationId,
    const std::string& method,
    const std::string& argsJson)
{
    return invokeAsyncBridge(
        asyncBridge_.get(),
        "logos inspector asynchronous bridge is not initialized",
        [&](LogosInspectorAsyncBridge& bridge) {
            return bridge.callAsync(correlationId, method, argsJson);
        });
}

std::string LogosInspectorImpl::callModuleAsync(
    const std::string& correlationId,
    const std::string& module,
    const std::string& method,
    const std::string& argsJson)
{
    return invokeAsyncBridge(
        asyncBridge_.get(),
        "logos inspector asynchronous bridge is not initialized",
        [&](LogosInspectorAsyncBridge& bridge) {
            return bridge.callModuleAsync(correlationId, module, method, argsJson);
        });
}

std::string LogosInspectorImpl::pollAsync(const std::string& token)
{
    return invokeAsyncBridge(
        asyncBridge_.get(),
        "logos inspector asynchronous bridge is not initialized",
        [&](LogosInspectorAsyncBridge& bridge) { return bridge.pollAsync(token); });
}

std::string LogosInspectorImpl::cancelAsync(const std::string& token)
{
    return invokeAsyncBridge(
        asyncBridge_.get(),
        "logos inspector asynchronous bridge is not initialized",
        [&](LogosInspectorAsyncBridge& bridge) { return bridge.cancelAsync(token); });
}

std::string LogosInspectorImpl::releaseAsync(const std::string& token)
{
    return invokeAsyncBridge(
        asyncBridge_.get(),
        "logos inspector asynchronous bridge is not initialized",
        [&](LogosInspectorAsyncBridge& bridge) { return bridge.releaseAsync(token); });
}

std::string LogosInspectorImpl::asyncBridgeSchema()
{
    return asyncBridge_
        ? "logos-inspector-async-bridge/v1"
        : "logos-inspector-async-bridge/unavailable";
}

bool LogosInspectorImpl::logosInspectorOwnsRuntimeModuleEvents()
{
    return asyncBridge_ != nullptr
        && asyncBridge_->ownsRuntimeModuleEvents();
}

std::string LogosInspectorImpl::moduleVersion()
{
    return kModuleVersion;
}
