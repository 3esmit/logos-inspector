#ifndef LOGOS_INSTANCE_H
#define LOGOS_INSTANCE_H

#include <QDebug>
#include <QString>
#include <QUuid>

/**
 * @brief LogosInstance provides a shared identifier for all processes launched
 * by Logos core
 *
 * The root process generates a UUID-based ID and exports it via the
 * LOGOS_INSTANCE_ID environment variable. Child processes inherit the variable
 * and return the same ID, so all processes in the application tree share one
 * ID. Used by both provider and consumer to build matching registry URLs.
 */
namespace LogosInstance {
    inline const QString id() {
        const QByteArray inherited = qgetenv("LOGOS_INSTANCE_ID");
        if (!inherited.isEmpty())
            return QString::fromUtf8(inherited);
        const QString newId = QUuid::createUuid().toString(QUuid::Id128).left(12);
        qputenv("LOGOS_INSTANCE_ID", newId.toUtf8());
        return newId;
    }

    inline QString id(const QString& moduleName) {
        return QString("local:logos_%1_%2").arg(moduleName).arg(id());
    }
}

#endif // LOGOS_INSTANCE_H
