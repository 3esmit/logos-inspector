#pragma once

#include <string>

#include "logos_inspector_core.h"
#include "logos_module_context.h"

class LogosInspectorImpl : public LogosModuleContext
{
public:
    LogosInspectorImpl();
    ~LogosInspectorImpl();

    LogosInspectorImpl(const LogosInspectorImpl&) = delete;
    LogosInspectorImpl& operator=(const LogosInspectorImpl&) = delete;

    /// Calls a Logos Inspector method with a JSON array argument string.
    std::string call(const std::string& method, const std::string& argsJson);

    /// Calls any module through the shared inspector bridge.
    std::string callModule(const std::string& module, const std::string& method, const std::string& argsJson);

    /// Returns this module package version.
    std::string moduleVersion();

private:
    LogosInspectorCore* core_ = nullptr;
};
