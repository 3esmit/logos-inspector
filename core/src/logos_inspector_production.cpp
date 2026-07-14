#include "logos_inspector_impl.h"

#include "logos_protocol_host_transport.h"

#include <memory>

LogosInspectorImpl::LogosInspectorImpl()
    : LogosInspectorImpl([] {
        return std::make_unique<LogosProtocolHostTransport>();
    })
{
}
