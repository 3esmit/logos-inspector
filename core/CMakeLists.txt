cmake_minimum_required(VERSION 3.14)
project(LogosInspectorCoreModule LANGUAGES CXX)

if(DEFINED ENV{LOGOS_MODULE_BUILDER_ROOT})
    include($ENV{LOGOS_MODULE_BUILDER_ROOT}/cmake/LogosModule.cmake)
else()
    message(FATAL_ERROR "LogosModule.cmake not found. Set LOGOS_MODULE_BUILDER_ROOT.")
endif()

file(READ "${CMAKE_CURRENT_SOURCE_DIR}/metadata.json" METADATA_JSON)
string(JSON MODULE_NAME GET "${METADATA_JSON}" name)

logos_module(
    NAME ${MODULE_NAME}
    SOURCES
        src/logos_inspector_impl.h
        src/logos_inspector_impl.cpp
    EXTERNAL_LIBS
        logos_inspector_core
)
