#ifndef LOGOS_ASYNC_OUTCOME_H
#define LOGOS_ASYNC_OUTCOME_H

#include "logos_call_error.h"

#include <QVariant>

#include <string>
#include <utility>

namespace logos {

struct AsyncCallOutcome {
    QVariant value;
    CallError error;

    bool ok() const { return error.ok(); }

    static AsyncCallOutcome success(QVariant value = QVariant())
    {
        return { std::move(value), {} };
    }

    static AsyncCallOutcome failure(std::string code,
                                    std::string message,
                                    std::string origin)
    {
        return { {}, { std::move(code), std::move(message), std::move(origin) } };
    }
};

} // namespace logos

#endif // LOGOS_ASYNC_OUTCOME_H
