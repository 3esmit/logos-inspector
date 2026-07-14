cmake_minimum_required(VERSION 3.14)
project(LogosInspectorCoreModule LANGUAGES CXX)

option(
    LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE
    "Enable the experimental host-transport async bridge"
    OFF
)

if(DEFINED ENV{LOGOS_MODULE_BUILDER_ROOT})
    include($ENV{LOGOS_MODULE_BUILDER_ROOT}/cmake/LogosModule.cmake)
else()
    message(FATAL_ERROR "LogosModule.cmake not found. Set LOGOS_MODULE_BUILDER_ROOT.")
endif()

file(READ "${CMAKE_CURRENT_SOURCE_DIR}/metadata.json" METADATA_JSON)
string(JSON MODULE_NAME GET "${METADATA_JSON}" name)
string(JSON MODULE_VERSION GET "${METADATA_JSON}" version)

logos_module(
    NAME ${MODULE_NAME}
    SOURCES
        src/logos_inspector_async_bridge.h
        src/logos_inspector_async_bridge.cpp
        src/logos_inspector_impl.h
        src/logos_inspector_impl.cpp
    EXTERNAL_LIBS
        logos_inspector_core
)

target_compile_features(${MODULE_NAME}_module_plugin PRIVATE cxx_std_17)

target_compile_definitions(${MODULE_NAME}_module_plugin
    PRIVATE
        LOGOS_INSPECTOR_MODULE_VERSION="${MODULE_VERSION}"
        LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE=$<BOOL:${LOGOS_INSPECTOR_ENABLE_ASYNC_BRIDGE}>
)
