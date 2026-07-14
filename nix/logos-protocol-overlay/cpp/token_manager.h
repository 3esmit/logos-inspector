#ifndef TOKEN_MANAGER_H
#define TOKEN_MANAGER_H

#include <QObject>
#include <QString>
#include <QHash>
#include <QMutex>
#include <QByteArray>
#include <QCryptographicHash>
#include <string>

/**
 * @brief Render a capability/auth token safe to write to logs.
 *
 * Tokens gate all cross-module RPC and are accepted by value (see
 * ModuleProxy::isAuthorized), so a raw token recovered from a log line is
 * directly replayable. This collapses a token to a non-reversible, non-
 * replayable fingerprint — a fixed prefix plus the first bytes of its SHA-256 —
 * suitable for correlating log lines without exposing the secret. The
 * "redacted:" prefix signals to anyone reading the log that this is a
 * deliberately non-replayable fingerprint, not a truncated real token. Empty
 * tokens render as "<none>" so missing-token cases stay greppable.
 *
 * Always pass tokens through this before logging; never log the raw value.
 */
inline QString redactToken(const QString& token)
{
    if (token.isEmpty()) {
        return QStringLiteral("<none>");
    }
    const QByteArray digest =
        QCryptographicHash::hash(token.toUtf8(), QCryptographicHash::Sha256);
    return QStringLiteral("redacted:") + QString::fromLatin1(digest.toHex().left(8)) + QStringLiteral("…");
}

/**
 * @brief TokenManager provides a singleton interface for managing authentication tokens
 * 
 * This class manages a collection of tokens identified by keys, providing thread-safe
 * access to store, retrieve, and manage tokens throughout the application lifecycle.
 */
class TokenManager : public QObject
{
    Q_OBJECT

public:
    /**
     * @brief Get the singleton instance of TokenManager
     * @return TokenManager& Reference to the singleton instance
     */
    static TokenManager& instance();

    /**
     * @brief Save a token with the given key
     * @param key The identifier for the token
     * @param token The token value to store
     */
    void saveToken(const QString& key, const QString& token);

    /**
     * @brief Save a token — const char* overload (resolves ambiguity, delegates to QString)
     */
    void saveToken(const char* key, const char* token)
        { saveToken(QString(key), QString(token)); }

    /**
     * @brief Save a token with the given key (std::string overload)
     */
    void saveToken(const std::string& key, const std::string& token);

    /**
     * @brief Retrieve a token by key
     * @param key The identifier for the token
     * @return QString The token value, or empty string if not found
     */
    QString getToken(const QString& key) const;

    /**
     * @brief Retrieve a token — const char* overload (resolves ambiguity, delegates to QString)
     */
    QString getToken(const char* key) const
        { return getToken(QString(key)); }

    /**
     * @brief Retrieve a token by key (std::string overload)
     * @return std::string The token value, or empty string if not found
     */
    std::string getToken(const std::string& key) const;

    /**
     * @brief Check if a token exists for the given key
     * @param key The identifier to check
     * @return bool True if token exists, false otherwise
     */
    bool hasToken(const QString& key) const;

    /**
     * @brief hasToken — const char* overload (resolves ambiguity, delegates to QString)
     */
    bool hasToken(const char* key) const
        { return hasToken(QString(key)); }

    /**
     * @brief Check if a token exists for the given key (std::string overload)
     */
    bool hasToken(const std::string& key) const;

    /**
     * @brief Remove a token by key
     * @param key The identifier for the token to remove
     * @return bool True if token was removed, false if it didn't exist
     */
    bool removeToken(const QString& key);

    /**
     * @brief removeToken — const char* overload (resolves ambiguity, delegates to QString)
     */
    bool removeToken(const char* key)
        { return removeToken(QString(key)); }

    /**
     * @brief Remove a token by key (std::string overload)
     */
    bool removeToken(const std::string& key);

    /**
     * @brief Clear all tokens
     */
    void clearAllTokens();

    /**
     * @brief Get all token keys
     * @return QList<QString> List of all token keys
     */
    QList<QString> getTokenKeys() const;

    /**
     * @brief Get the number of stored tokens
     * @return int Number of tokens stored
     */
    int tokenCount() const;

signals:
    /**
     * @brief Emitted when a token is saved
     * @param key The key of the saved token
     */
    void tokenSaved(const QString& key);

    /**
     * @brief Emitted when a token is removed
     * @param key The key of the removed token
     */
    void tokenRemoved(const QString& key);

    /**
     * @brief Emitted when all tokens are cleared
     */
    void allTokensCleared();

private:
    /**
     * @brief Private constructor for singleton pattern
     * @param parent Parent QObject
     */
    explicit TokenManager(QObject *parent = nullptr);

    /**
     * @brief Private destructor
     */
    ~TokenManager();

    // Delete copy constructor and assignment operator to enforce singleton
    TokenManager(const TokenManager&) = delete;
    TokenManager& operator=(const TokenManager&) = delete;

    /**
     * @brief Hash map storing tokens by key
     */
    QHash<QString, QString> m_tokens;

    /**
     * @brief Mutex for thread-safe access to tokens
     */
    mutable QMutex m_mutex;
};

#endif // TOKEN_MANAGER_H 
