@PACKAGE_INIT@

cmake_minimum_required(VERSION 3.22)

if(readcon-db_FOUND)
    return()
endif()

get_filename_component(READCON_DB_PREFIX_DIR "${CMAKE_CURRENT_LIST_DIR}/@PACKAGE_RELATIVE_PATH@" ABSOLUTE)

if (WIN32)
    set(READCON_DB_SHARED_LOCATION ${READCON_DB_PREFIX_DIR}/@BIN_INSTALL_DIR@/@READCON_DB_SHARED_LIB_NAME@)
    set(READCON_DB_IMPLIB_LOCATION ${READCON_DB_PREFIX_DIR}/@LIB_INSTALL_DIR@/@READCON_DB_IMPLIB_NAME@)
else()
    set(READCON_DB_SHARED_LOCATION ${READCON_DB_PREFIX_DIR}/@LIB_INSTALL_DIR@/@READCON_DB_SHARED_LIB_NAME@)
endif()

set(READCON_DB_STATIC_LOCATION ${READCON_DB_PREFIX_DIR}/@LIB_INSTALL_DIR@/@READCON_DB_STATIC_LIB_NAME@)
set(READCON_DB_INCLUDE ${READCON_DB_PREFIX_DIR}/@INCLUDE_INSTALL_DIR@/)

if (NOT EXISTS ${READCON_DB_INCLUDE}/readcon-db.h)
    message(FATAL_ERROR
        "could not find readcon-db.h in '${READCON_DB_INCLUDE}'. "
        "Re-install readcon-db (headers are shipped in the source tree; cbindgen is not required).")
endif()

# Shared library target
if (@READCON_DB_INSTALL_BOTH_STATIC_SHARED@ OR @BUILD_SHARED_LIBS@)
    if (NOT EXISTS ${READCON_DB_SHARED_LOCATION})
        message(FATAL_ERROR "could not find readcon-db shared library at '${READCON_DB_SHARED_LOCATION}'")
    endif()

    add_library(readcon-db::shared SHARED IMPORTED GLOBAL)
    set_target_properties(readcon-db::shared PROPERTIES
        IMPORTED_LOCATION ${READCON_DB_SHARED_LOCATION}
        INTERFACE_INCLUDE_DIRECTORIES ${READCON_DB_INCLUDE}
        BUILD_VERSION "@PROJECT_VERSION@"
    )

    if (WIN32)
        if (NOT EXISTS ${READCON_DB_IMPLIB_LOCATION})
            message(FATAL_ERROR "could not find readcon-db import library at '${READCON_DB_IMPLIB_LOCATION}'")
        endif()
        set_target_properties(readcon-db::shared PROPERTIES
            IMPORTED_IMPLIB ${READCON_DB_IMPLIB_LOCATION}
        )
    endif()
endif()

# Static library target
if (@READCON_DB_INSTALL_BOTH_STATIC_SHARED@ OR NOT @BUILD_SHARED_LIBS@)
    if (NOT EXISTS ${READCON_DB_STATIC_LOCATION})
        message(FATAL_ERROR "could not find readcon-db static library at '${READCON_DB_STATIC_LOCATION}'")
    endif()

    add_library(readcon-db::static STATIC IMPORTED GLOBAL)
    set_target_properties(readcon-db::static PROPERTIES
        IMPORTED_LOCATION ${READCON_DB_STATIC_LOCATION}
        INTERFACE_INCLUDE_DIRECTORIES ${READCON_DB_INCLUDE}
        INTERFACE_LINK_LIBRARIES "@CARGO_DEFAULT_LIBRARIES@"
        BUILD_VERSION "@PROJECT_VERSION@"
    )
endif()

if (@BUILD_SHARED_LIBS@)
    if (NOT TARGET readcon-db::shared)
        message(FATAL_ERROR "readcon-db was installed without a shared library")
    endif()
    add_library(readcon-db ALIAS readcon-db::shared)
    add_library(readcon-db::readcon-db ALIAS readcon-db::shared)
else()
    if (NOT TARGET readcon-db::static)
        message(FATAL_ERROR "readcon-db was installed without a static library")
    endif()
    add_library(readcon-db ALIAS readcon-db::static)
    add_library(readcon-db::readcon-db ALIAS readcon-db::static)
endif()
