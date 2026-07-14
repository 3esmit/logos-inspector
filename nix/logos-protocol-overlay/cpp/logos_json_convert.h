#ifndef LOGOS_JSON_CONVERT_H
#define LOGOS_JSON_CONVERT_H

#include <QVariant>
#include <QVariantList>
#include <QJsonArray>
#include <nlohmann/json.hpp>
#include <string>
#include <vector>

struct LogosMethodMetadata {
    std::string name;
    std::string signature;
    std::string returnType;
    bool        isInvokable = true;
    nlohmann::json parameters = nlohmann::json::array();
};

namespace logos {

nlohmann::json qvariantToNlohmann(const QVariant& v);
QVariant nlohmannToQVariant(const nlohmann::json& j);
QVariantList nlohmannArgsToQVariantList(const nlohmann::json& args);
QJsonArray methodsToJsonArray(const std::vector<LogosMethodMetadata>& methods);

} // namespace logos

#endif // LOGOS_JSON_CONVERT_H
