#include "logos_inspector_impl.h"

namespace {
#ifndef LOGOS_INSPECTOR_MODULE_VERSION
#define LOGOS_INSPECTOR_MODULE_VERSION "unknown"
#endif

constexpr const char* kModuleVersion = LOGOS_INSPECTOR_MODULE_VERSION;

std::string jsonError(const std::string& error)
{
    std::string escaped;
    escaped.reserve(error.size());
    for (const char ch : error) {
        switch (ch) {
        case '\\':
            escaped += "\\\\";
            break;
        case '"':
            escaped += "\\\"";
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
            escaped += ch;
            break;
        }
    }
    return "{\"ok\":false,\"value\":null,\"text\":\"\",\"error\":\"" + escaped + "\"}";
}

std::string takeResponse(char* response)
{
    if (response == nullptr) {
        return jsonError("logos inspector core returned no response");
    }
    std::string value(response);
    logos_inspector_core_string_free(response);
    return value;
}
}

LogosInspectorImpl::LogosInspectorImpl()
    : core_(logos_inspector_core_new())
{
}

LogosInspectorImpl::~LogosInspectorImpl()
{
    logos_inspector_core_free(core_);
    core_ = nullptr;
}

std::string LogosInspectorImpl::call(const std::string& method, const std::string& argsJson)
{
    if (core_ == nullptr) {
        return jsonError("logos inspector core is not initialized");
    }
    return takeResponse(logos_inspector_core_call(core_, method.c_str(), argsJson.c_str()));
}

std::string LogosInspectorImpl::callModule(
    const std::string& module,
    const std::string& method,
    const std::string& argsJson)
{
    if (core_ == nullptr) {
        return jsonError("logos inspector core is not initialized");
    }
    return takeResponse(logos_inspector_core_call_module(
        core_,
        module.c_str(),
        method.c_str(),
        argsJson.c_str()));
}

std::string LogosInspectorImpl::moduleVersion()
{
    return kModuleVersion;
}
