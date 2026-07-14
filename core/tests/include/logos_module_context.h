#pragma once

#include <string>

class LogosModuleContext
{
public:
    virtual ~LogosModuleContext() = default;

    void _logosCoreSetContext_(std::string, std::string, std::string)
    {
        contextReady_ = true;
        onContextReady();
    }

    bool isContextReady() const noexcept
    {
        return contextReady_;
    }

protected:
    virtual void onContextReady() {}

private:
    bool contextReady_ = false;
};
