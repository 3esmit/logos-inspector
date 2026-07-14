#include "logos_inspector_impl.h"

#include "logos_inspector_async_bridge.h"

#include <exception>
#include <utility>

namespace {
#ifndef LOGOS_INSPECTOR_MODULE_VERSION
#define LOGOS_INSPECTOR_MODULE_VERSION "unknown"
#endif
#ifndef LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE
#define LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE 0
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

#if !LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE
std::string takeResponse(char* response)
{
    if (response == nullptr) {
        return jsonError("logos inspector core returned no response");
    }
    std::string value(response);
    logos_inspector_core_string_free(response);
    return value;
}
#endif

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

LogosInspectorImpl::LogosInspectorImpl()
{
#if LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE
    try {
        asyncBridge_ = std::make_unique<LogosInspectorAsyncBridge>();
    } catch (...) {
        asyncBridge_.reset();
    }
#else
    legacyCore_ = logos_inspector_core_new();
#endif
}

LogosInspectorImpl::~LogosInspectorImpl()
{
#if LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE
    asyncBridge_.reset();
#else
    logos_inspector_core_free(legacyCore_);
    legacyCore_ = nullptr;
#endif
}

std::string LogosInspectorImpl::call(const std::string& method, const std::string& argsJson)
{
#if LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE
    return invokeAsyncBridge(
        asyncBridge_.get(),
        "logos inspector bridge is not initialized",
        [&](LogosInspectorAsyncBridge& bridge) { return bridge.call(method, argsJson); });
#else
    if (legacyCore_ == nullptr) {
        return jsonError("logos inspector core is not initialized");
    }
    return takeResponse(logos_inspector_core_call(
        legacyCore_,
        method.c_str(),
        argsJson.c_str()));
#endif
}

std::string LogosInspectorImpl::callModule(
    const std::string& module,
    const std::string& method,
    const std::string& argsJson)
{
#if LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE
    static_cast<void>(module);
    static_cast<void>(method);
    static_cast<void>(argsJson);
    return jsonError("synchronous module calls are unsupported; use callModuleAsync");
#else
    if (legacyCore_ == nullptr) {
        return jsonError("logos inspector core is not initialized");
    }
    return takeResponse(logos_inspector_core_call_module(
        legacyCore_,
        module.c_str(),
        method.c_str(),
        argsJson.c_str()));
#endif
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
#if LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE
    return asyncBridge_
        ? "logos-inspector-async-bridge/v1"
        : "logos-inspector-async-bridge/unavailable";
#else
    return "";
#endif
}

std::string LogosInspectorImpl::moduleVersion()
{
    return kModuleVersion;
}
