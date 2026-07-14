#ifndef LOGOS_MODE_H
#define LOGOS_MODE_H

#include <QDebug>

/**
 * @brief LogosMode defines the communication mode for the SDK
 * 
 * - Remote: Uses QRemoteObjects for inter-process communication (desktop apps)
 * - Local: Uses in-process PluginRegistry (mobile apps, single process)
 * - Mock: Uses in-memory mock transport for unit testing
 */
enum class LogosMode {
    Remote,  // Default: Use QRemoteObjects (IPC)
    Local,   // Use in-process PluginRegistry
    Mock     // Use in-memory mock transport for unit testing
};

/**
 * @brief Timeout provides a strongly-typed wrapper for timeout values
 * 
 * This prevents implicit conversion from int, avoiding ambiguity with
 * QVariant parameters in method overloads.
 * 
 * Example usage:
 *   invokeRemoteMethod("module", "method", arg1, arg2, Timeout(5000));
 */
struct Timeout {
    int ms;
    explicit Timeout(int milliseconds = 20000) : ms(milliseconds) {}
};

/**
 * @brief LogosModeConfig provides global mode configuration
 * 
 * Set the mode once at application startup before creating any LogosAPI instances.
 * 
 * Example usage:
 *   LogosModeConfig::setMode(LogosMode::Local);  // For mobile apps
 *   LogosAPI api("my_module");  // Will use local mode
 */
namespace LogosModeConfig {

    /**
     * @brief Get the current mode (internal storage)
     * @return Reference to the static mode variable
     */
    inline LogosMode& modeStorage() {
        static LogosMode mode = LogosMode::Remote;  // Default to Remote
        return mode;
    }

    /**
     * @brief Set the SDK communication mode
     * @param mode The mode to use (Remote or Local)
     * 
     * Call this before creating any LogosAPI instances.
     */
    inline void setMode(LogosMode mode) {
        modeStorage() = mode;
        QString modeName = (mode == LogosMode::Local) ? "Local"
                         : (mode == LogosMode::Mock)  ? "Mock"
                                                       : "Remote";
        qDebug() << "LogosModeConfig: Mode set to" << modeName;
    }

    /**
     * @brief Get the current SDK communication mode
     * @return The current mode
     */
    inline LogosMode getMode() {
        return modeStorage();
    }

    /**
     * @brief Check if the SDK is in Local mode
     * @return true if Local mode, false if Remote mode
     */
    inline bool isLocal() {
        return modeStorage() == LogosMode::Local;
    }

    /**
     * @brief Check if the SDK is in Remote mode
     * @return true if Remote mode, false if Local mode
     */
    inline bool isRemote() {
        return modeStorage() == LogosMode::Remote;
    }

    /**
     * @brief Check if the SDK is in Mock mode
     * @return true if Mock mode, false otherwise
     */
    inline bool isMock() {
        return modeStorage() == LogosMode::Mock;
    }

}

#endif // LOGOS_MODE_H
