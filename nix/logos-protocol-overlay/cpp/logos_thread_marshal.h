#ifndef LOGOS_THREAD_MARSHAL_H
#define LOGOS_THREAD_MARSHAL_H

#include <type_traits>

#include <QMetaObject>
#include <QObject>
#include <QThread>

namespace logos {

// Run `fn` on `obj`'s (owner) thread, blocking the caller until it completes,
// and forward the return value. If already on that thread, runs directly with
// no marshaling and no overhead (the common case).
//
// Why: Logos inter-module calls go over Qt Remote Objects, whose replicas only
// work on the thread that owns them (the module's main/event-loop thread). This
// lets a module call other modules from a worker thread (e.g. an HTTP server
// thread) without the module touching Qt — the SDK transparently marshals the
// call onto the owner thread.
//
// Requirements:
//   - `obj`'s thread must be running an event loop (it is — the module's main
//     thread runs QCoreApplication::exec()). The same-thread guard avoids the
//     BlockingQueuedConnection self-deadlock.
//   - The return type must be void or default-constructible (the marshaled
//     branch holds the result in a local before assigning it), and must not be
//     a reference (there'd be nothing to bind the local to). Both are satisfied
//     by the SDK's uses here (void, QVariant, LogosObject*, LogosAPIClient*).
template <typename Fn>
auto runOnOwnerThread(QObject* obj, Fn&& fn) -> decltype(fn())
{
    using Ret = decltype(fn());
    static_assert(!std::is_reference_v<Ret>,
                  "runOnOwnerThread does not support reference return types");
    if (QThread::currentThread() == obj->thread()) {
        return fn();
    }
    if constexpr (std::is_void_v<Ret>) {
        QMetaObject::invokeMethod(obj, [&]() { fn(); }, Qt::BlockingQueuedConnection);
        return;
    } else {
        Ret ret{};
        QMetaObject::invokeMethod(obj, [&]() { ret = fn(); }, Qt::BlockingQueuedConnection);
        return ret;
    }
}

}  // namespace logos

#endif  // LOGOS_THREAD_MARSHAL_H
