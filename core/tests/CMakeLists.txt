cmake_minimum_required(VERSION 3.14)
project(LogosInspectorAsyncBridgeTests LANGUAGES CXX)

enable_testing()
find_package(Threads REQUIRED)
find_package(Qt6 REQUIRED COMPONENTS Core)

function(add_async_bridge_test target)
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
    )
    target_include_directories(${target} PRIVATE
        include
        ../src
        ../../crates/core-ffi/include
    )
    target_link_libraries(${target} PRIVATE Threads::Threads)
    add_test(NAME ${target} COMMAND ${target})
endfunction()

add_async_bridge_test(logos_inspector_async_bridge_tests)

add_executable(logos_protocol_host_transport_tests
    test_logos_protocol_host_transport.cpp
    ../src/logos_protocol_host_transport.cpp
)
target_compile_features(logos_protocol_host_transport_tests PRIVATE cxx_std_17)
target_compile_options(logos_protocol_host_transport_tests PRIVATE
    -Wall
    -Wextra
    -Wpedantic
    -Werror
)
target_include_directories(logos_protocol_host_transport_tests PRIVATE
    ../src
    ../../crates/core-ffi/include
    ../../nix/logos-protocol-overlay/cpp
)
target_link_libraries(logos_protocol_host_transport_tests PRIVATE
    Qt6::Core
    Threads::Threads
)
add_test(NAME logos_protocol_host_transport_tests COMMAND logos_protocol_host_transport_tests)
set_tests_properties(logos_protocol_host_transport_tests PROPERTIES TIMEOUT 15)

option(
    LOGOS_INSPECTOR_ENABLE_COMPOSED_HOST_TESTS
    "Build the real Rust core + logos-protocol host composition regression"
    OFF
)

if(LOGOS_INSPECTOR_ENABLE_COMPOSED_HOST_TESTS)
    if(LOGOS_PROTOCOL_ROOT)
        find_package(
            logos-protocol
            REQUIRED
            CONFIG
            PATHS "${LOGOS_PROTOCOL_ROOT}/lib/cmake/logos-protocol"
            NO_DEFAULT_PATH
        )
    else()
        find_package(logos-protocol REQUIRED CONFIG)
    endif()

    set(_logos_inspector_core_include_hints
        "${CMAKE_CURRENT_SOURCE_DIR}/../lib"
    )
    set(_logos_inspector_core_library_hints
        "${CMAKE_CURRENT_SOURCE_DIR}/../lib"
    )
    if(LOGOS_INSPECTOR_CORE_ROOT)
        list(PREPEND
            _logos_inspector_core_include_hints
            "${LOGOS_INSPECTOR_CORE_ROOT}/include"
        )
        list(PREPEND
            _logos_inspector_core_library_hints
            "${LOGOS_INSPECTOR_CORE_ROOT}/lib"
        )
    endif()

    find_path(
        LOGOS_INSPECTOR_CORE_INCLUDE_DIR
        NAMES logos_inspector_core.h
        PATHS ${_logos_inspector_core_include_hints}
        NO_DEFAULT_PATH
    )
    find_library(
        LOGOS_INSPECTOR_CORE_LIBRARY
        NAMES logos_inspector_core
        PATHS ${_logos_inspector_core_library_hints}
        NO_DEFAULT_PATH
    )
    if(NOT LOGOS_INSPECTOR_CORE_INCLUDE_DIR OR NOT LOGOS_INSPECTOR_CORE_LIBRARY)
        message(FATAL_ERROR
            "Real Logos Inspector core FFI was not found. Set "
            "LOGOS_INSPECTOR_CORE_ROOT or stage its header/library in core/lib."
        )
    endif()

    add_library(logos_inspector_core_ffi UNKNOWN IMPORTED)
    set_target_properties(logos_inspector_core_ffi PROPERTIES
        IMPORTED_LOCATION "${LOGOS_INSPECTOR_CORE_LIBRARY}"
        INTERFACE_INCLUDE_DIRECTORIES "${LOGOS_INSPECTOR_CORE_INCLUDE_DIR}"
    )

    # Module-builder test sources omit the workspace crates directory. Keep the
    # fake-core unit targets on their existing implementations while sourcing
    # the public ABI declaration from the same staged core package.
    target_include_directories(logos_inspector_async_bridge_tests PRIVATE
        "${LOGOS_INSPECTOR_CORE_INCLUDE_DIR}"
    )
    target_include_directories(logos_protocol_host_transport_tests PRIVATE
        "${LOGOS_INSPECTOR_CORE_INCLUDE_DIR}"
    )

    add_executable(logos_inspector_composed_host_integration_tests
        test_logos_inspector_composed_host_integration.cpp
        ../src/logos_inspector_async_bridge.cpp
        ../src/logos_protocol_host_transport.cpp
    )
    target_compile_features(
        logos_inspector_composed_host_integration_tests
        PRIVATE
        cxx_std_17
    )
    target_compile_options(
        logos_inspector_composed_host_integration_tests
        PRIVATE
        -Wall
        -Wextra
        -Wpedantic
        -Werror
    )
    target_include_directories(
        logos_inspector_composed_host_integration_tests
        PRIVATE
        ../src
    )
    target_link_libraries(
        logos_inspector_composed_host_integration_tests
        PRIVATE
        Threads::Threads
        logos-protocol::logos_protocol
        logos_inspector_core_ffi
    )
    add_test(
        NAME logos_inspector_composed_host_integration_tests
        COMMAND logos_inspector_composed_host_integration_tests
    )
    set_tests_properties(
        logos_inspector_composed_host_integration_tests
        PROPERTIES TIMEOUT 20
    )
endif()
