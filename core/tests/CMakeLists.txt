cmake_minimum_required(VERSION 3.14)
project(LogosInspectorAsyncBridgeTests LANGUAGES CXX)

enable_testing()
find_package(Threads REQUIRED)

function(add_async_bridge_test target enable_async_bridge)
    add_executable(${target}
        test_logos_inspector_async_bridge.cpp
        ../src/logos_inspector_async_bridge.cpp
        ../src/logos_inspector_impl.cpp
    )
    target_compile_features(${target} PRIVATE cxx_std_17)
    target_compile_options(${target} PRIVATE
        -Wall
        -Wextra
        -Wpedantic
        -Werror
    )
    target_compile_definitions(${target} PRIVATE
        LOGOS_INSPECTOR_MODULE_VERSION="test"
        LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE=${enable_async_bridge}
    )
    target_include_directories(${target} PRIVATE
        include
        ../src
        ../../crates/core-ffi/include
    )
    target_link_libraries(${target} PRIVATE Threads::Threads)
    add_test(NAME ${target} COMMAND ${target})
endfunction()

add_async_bridge_test(logos_inspector_async_bridge_tests 1)
add_async_bridge_test(logos_inspector_legacy_wrapper_tests 0)
